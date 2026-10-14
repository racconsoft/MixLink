use std::sync::{Arc, Mutex};
use std::time::Duration;

use mixlink_core::model::{Channel, Config, MixKind, MixTarget, State};
use mixlink_core::protocol::{Command, ErrorCode, Event, Request};
use mixlink_desktop::link::{DaemonLink, LinkSink, NOT_RUNNING};
use mixlinkd::actor::{ActorChannels, run_actor};
use mixlinkd::app::App;
use mixlinkd::backend::BackendEvent;
use mixlinkd::backend::fake::FakeBackend;
use mixlinkd::ipc::{bind_socket, serve};
use mixlinkd::mixer::dsp::Peaks;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Clone, PartialEq)]
enum Rec {
    Connected(bool),
    State(State),
    Meters,
}

#[derive(Default)]
struct Recorder(Mutex<Vec<Rec>>);

impl LinkSink for Recorder {
    fn state(&self, state: &State) {
        self.0.lock().unwrap().push(Rec::State(state.clone()));
    }
    fn meters(&self, _: &Event) {
        self.0.lock().unwrap().push(Rec::Meters);
    }
    fn connection(&self, connected: bool) {
        self.0.lock().unwrap().push(Rec::Connected(connected));
    }
}

impl Recorder {
    fn all(&self) -> Vec<Rec> {
        self.0.lock().unwrap().clone()
    }
    async fn wait_for(&self, what: &str, pred: impl Fn(&[Rec]) -> bool) {
        for _ in 0..200 {
            if pred(&self.all()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {what}: {:?}", self.all());
    }
}

fn config() -> Config {
    Config {
        version: 1,
        fallback_channel: "system".into(),
        listen: MixKind::Monitor,
        settings: Default::default(),
        mixes: Default::default(),
        channels: vec![
            Channel::new("system", "System"),
            Channel::new("music", "Music"),
        ],
    }
}

/// A real mixlinkd core (actor + IPC) with the fake audio backend.
/// Returns the backend-event sender, e.g. to feed meters.
async fn start_daemon(path: &std::path::Path) -> mpsc::UnboundedSender<BackendEvent> {
    let listener = bind_socket(path).unwrap();
    let (msg_tx, msgs) = mpsc::channel(16);
    let (ev_tx, backend_events) = mpsc::unbounded_channel();
    let (bc, _) = broadcast::channel(64);
    tokio::spawn(serve(listener, msg_tx, bc.clone()));
    let events = ev_tx.clone();
    tokio::spawn(async move {
        let _keep = ev_tx;
        run_actor(
            App::new(config(), FakeBackend::new(), false),
            ActorChannels {
                msgs,
                backend_events,
                broadcast: bc,
            },
            |_: &Config| {},
            Duration::from_millis(500),
        )
        .await
    });
    events
}

#[tokio::test(flavor = "multi_thread")]
async fn connects_loads_state_and_forwards_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixlink.sock");
    start_daemon(&path).await;
    let rec = Arc::new(Recorder::default());
    let link = DaemonLink::new();
    tokio::spawn(
        link.clone()
            .run(path, rec.clone(), Duration::from_millis(50)),
    );

    rec.wait_for("connected", |r| r.contains(&Rec::Connected(true)))
        .await;
    assert!(
        matches!(rec.all().iter().find(|r| matches!(r, Rec::State(_))), Some(Rec::State(s)) if s.config.channels.len() == 2),
        "initial state is loaded with get_state"
    );

    let result = link
        .send(Command::SetVolume {
            channel: "music".into(),
            mix: MixTarget::Stream,
            value: Some(40),
            delta: None,
        })
        .await
        .unwrap();
    assert_eq!(result["stream"]["volume"], 40);
    rec.wait_for("state_changed", |r| {
        r.iter().any(|x| matches!(x, Rec::State(s) if s.config.channel("music").unwrap().stream.volume == 40))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_disconnected_and_rejects_commands_without_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let link = DaemonLink::new();
    tokio::spawn(link.clone().run(
        dir.path().join("none.sock"),
        rec.clone(),
        Duration::from_millis(20),
    ));
    rec.wait_for("disconnected", |r| r.contains(&Rec::Connected(false)))
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        rec.all(),
        vec![Rec::Connected(false)],
        "reported once, not on every retry"
    );
    let err = link.send(Command::GetState).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(err.message, NOT_RUNNING);
}

/// Minimal fake daemon: answers subscribe/get_state, then hangs up after `hold`.
async fn flaky_daemon(path: std::path::PathBuf, volume: u8, hold: Duration) {
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    drop(listener);
    let (r, mut w) = stream.into_split();
    let mut lines = BufReader::new(r).lines();
    for _ in 0..2 {
        let req: Request =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let result = match req.command {
            Command::GetState => {
                let mut state = State {
                    config: config(),
                    apps: vec![],
                    devices: vec![],
                    warnings: vec![],
                };
                state.config.channels[1].stream.volume = volume;
                serde_json::to_value(state).unwrap()
            }
            _ => serde_json::Value::Null,
        };
        let line =
            serde_json::json!({ "id": req.id, "ok": true, "result": result }).to_string() + "\n";
        w.write_all(line.as_bytes()).await.unwrap();
    }
    tokio::time::sleep(hold).await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn reloads_full_state_after_the_daemon_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixlink.sock");
    let first = tokio::spawn(flaky_daemon(path.clone(), 11, Duration::from_millis(100)));
    let rec = Arc::new(Recorder::default());
    tokio::spawn(DaemonLink::new().run(path.clone(), rec.clone(), Duration::from_millis(30)));
    rec.wait_for("first state", |r| {
        r.iter()
            .any(|x| matches!(x, Rec::State(s) if s.config.channels[1].stream.volume == 11))
    })
    .await;
    first.await.unwrap();
    rec.wait_for("disconnect", |r| r.ends_with(&[Rec::Connected(false)]))
        .await;

    tokio::spawn(flaky_daemon(path.clone(), 22, Duration::from_secs(5)));
    rec.wait_for("state after restart", |r| {
        r.iter()
            .any(|x| matches!(x, Rec::State(s) if s.config.channels[1].stream.volume == 22))
    })
    .await;
    rec.wait_for("reconnected", |r| r.ends_with(&[Rec::Connected(true)]))
        .await;
}

fn meter_count(rec: &Recorder) -> usize {
    rec.all().iter().filter(|r| **r == Rec::Meters).count()
}

#[tokio::test(flavor = "multi_thread")]
async fn meters_can_be_switched_off_and_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixlink.sock");
    let backend = start_daemon(&path).await;
    let rec = Arc::new(Recorder::default());
    let link = DaemonLink::new();
    tokio::spawn(
        link.clone()
            .run(path, rec.clone(), Duration::from_millis(50)),
    );
    rec.wait_for("connected", |r| r.contains(&Rec::Connected(true)))
        .await;

    backend
        .send(BackendEvent::Meters(Peaks::default()))
        .unwrap();
    rec.wait_for("meters on by default", |r| r.contains(&Rec::Meters))
        .await;

    link.set_meters(false).await;
    // Let anything already in flight drain.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = meter_count(&rec);
    for _ in 0..5 {
        backend
            .send(BackendEvent::Meters(Peaks::default()))
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(meter_count(&rec), before, "no meters while switched off");

    link.set_meters(true).await;
    backend
        .send(BackendEvent::Meters(Peaks::default()))
        .unwrap();
    rec.wait_for("meters flow again", |r| {
        r.iter().filter(|x| **x == Rec::Meters).count() > before
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remembered_meter_flag_is_used_for_the_handshake() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixlink.sock");
    let backend = start_daemon(&path).await;
    let rec = Arc::new(Recorder::default());
    let link = DaemonLink::new();
    link.set_meters(false).await;
    tokio::spawn(
        link.clone()
            .run(path, rec.clone(), Duration::from_millis(50)),
    );
    rec.wait_for("connected", |r| r.contains(&Rec::Connected(true)))
        .await;
    for _ in 0..5 {
        backend
            .send(BackendEvent::Meters(Peaks::default()))
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(meter_count(&rec), 0, "subscribed without meters");
}
