use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::UnixStream;
use tokio::sync::{broadcast, mpsc};
use wavelink_core::model::{Channel, Config, MixKind};
use wavelink_core::protocol::{Event, Response};
use wavelinkd::actor::{run_actor, ActorChannels};
use wavelinkd::app::App;
use wavelinkd::backend::fake::FakeBackend;
use wavelinkd::ipc::{bind_socket, serve};

fn config() -> Config {
    Config {
        version: 1,
        fallback_channel: "system".into(),
        listen: MixKind::Monitor,
        settings: Default::default(),
        mixes: Default::default(),
        channels: vec![Channel::new("system", "System"), Channel::new("music", "Music")],
    }
}

/// Starts actor + server on a temp socket; returns the socket path guard.
async fn start() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wl.sock");
    let listener = bind_socket(&path).unwrap();
    let (msg_tx, msgs) = mpsc::channel(16);
    let (_ev_tx, backend_events) = mpsc::unbounded_channel();
    let (bc, _) = broadcast::channel(64);
    tokio::spawn(serve(listener, msg_tx, bc.clone()));
    tokio::spawn(async move {
        let _keep_events_open = _ev_tx;
        run_actor(
            App::new(config(), FakeBackend::new(), false),
            ActorChannels { msgs, backend_events, broadcast: bc },
            |_: &Config| {},
            Duration::from_millis(500),
        )
        .await
    });
    (dir, path)
}

async fn connect(path: &std::path::Path) -> (Lines<BufReader<OwnedReadHalf>>, tokio::net::unix::OwnedWriteHalf) {
    let (read, write) = UnixStream::connect(path).await.unwrap().into_split();
    (BufReader::new(read).lines(), write)
}

async fn request(
    lines: &mut Lines<BufReader<OwnedReadHalf>>,
    write: &mut tokio::net::unix::OwnedWriteHalf,
    json: &str,
) -> Response {
    write.write_all(format!("{json}\n").as_bytes()).await.unwrap();
    serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap()
}

#[tokio::test]
async fn changes_are_answered_and_broadcast_to_subscribers() {
    let (_dir, path) = start().await;
    let (mut sub_lines, mut sub_write) = connect(&path).await;
    let resp = request(&mut sub_lines, &mut sub_write, r#"{"id":1,"cmd":"subscribe","args":{"state":true}}"#).await;
    assert!(resp.ok);

    let (mut lines, mut write) = connect(&path).await;
    let resp = request(
        &mut lines,
        &mut write,
        r#"{"id":2,"cmd":"set_volume","args":{"channel":"music","mix":"stream","value":40}}"#,
    )
    .await;
    assert!(resp.ok);
    assert_eq!(resp.result.unwrap()["stream"]["volume"], 40);

    let event: Event = serde_json::from_str(&sub_lines.next_line().await.unwrap().unwrap()).unwrap();
    match event {
        Event::StateChanged { state } => assert_eq!(state.config.channel("music").unwrap().stream.volume, 40),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn invalid_json_gets_an_error_response() {
    let (_dir, path) = start().await;
    let (mut lines, mut write) = connect(&path).await;
    let resp = request(&mut lines, &mut write, "{not json").await;
    assert!(!resp.ok);
    assert_eq!(resp.id, 0);
    let resp = request(&mut lines, &mut write, r#"{"id":5,"cmd":"get_state"}"#).await;
    assert!(resp.ok, "connection stays usable");
}

#[tokio::test]
async fn invalid_request_echoes_recovered_id() {
    let (_dir, path) = start().await;
    let (mut lines, mut write) = connect(&path).await;
    let resp = request(&mut lines, &mut write, r#"{"id":42,"cmd":"set_vloume","args":{}}"#).await;
    assert!(!resp.ok);
    assert_eq!(resp.id, 42);
    assert_eq!(resp.error.unwrap().code, wavelink_core::protocol::ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn unknown_channel_error_lists_options() {
    let (_dir, path) = start().await;
    let (mut lines, mut write) = connect(&path).await;
    let resp = request(
        &mut lines,
        &mut write,
        r#"{"id":3,"cmd":"set_mute","args":{"channel":"xyz","mix":"both","muted":true}}"#,
    )
    .await;
    let error = resp.error.unwrap();
    assert_eq!(error.options, vec!["System", "Music"]);
}

#[tokio::test]
async fn stale_socket_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wl.sock");
    std::fs::write(&path, "stale").unwrap();
    bind_socket(&path).unwrap();
}

#[tokio::test]
async fn second_instance_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wl.sock");
    let _first = bind_socket(&path).unwrap();
    let err = bind_socket(&path).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    assert!(err.to_string().contains("already running"));
}

#[tokio::test]
async fn socket_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wl.sock");
    let _l = bind_socket(&path).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}
