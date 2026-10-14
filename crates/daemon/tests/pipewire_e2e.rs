mod support;

use std::time::Duration;

use tokio::sync::mpsc;
use wavelinkd::backend::{AudioBackend, BackendCommand, BackendEvent, ChannelSource};
use wavelinkd::mixer::dsp::{Gains, Targets};
use wavelinkd::pw::PipeWireBackend;

async fn wait_for(
    rx: &mut mpsc::UnboundedReceiver<BackendEvent>,
    what: &str,
    pred: impl Fn(&BackendEvent) -> bool,
) -> BackendEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("backend alive");
            assert_ne!(
                event,
                BackendEvent::Disconnected,
                "session disconnected while waiting for {what}"
            );
            if pred(&event) {
                return event;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// Drains everything queued so far; the session must never have disconnected.
fn drain_checked(rx: &mut mpsc::UnboundedReceiver<BackendEvent>) -> Vec<BackendEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        assert_ne!(event, BackendEvent::Disconnected, "session disconnected");
        events.push(event);
    }
    events
}

/// Kills the helper processes if the test panics, so they neither keep the test
/// harness' output pipe open nor leak streams into later runs.
struct Reaper(Vec<u32>);

impl Drop for Reaper {
    fn drop(&mut self) {
        if std::thread::panicking() {
            for pid in &self.0 {
                let _ = std::process::Command::new("kill")
                    .args(["-INT", &pid.to_string()])
                    .status();
            }
        }
    }
}

/// Whether `pw-link -l` lists a link from port `output` to port `input`.
fn has_link(output: &str, input: &str) -> bool {
    let out = std::process::Command::new("pw-link")
        .arg("-l")
        .output()
        .expect("pw-link");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim() == output {
            for linked in lines.by_ref().take_while(|l| l.starts_with(' ')) {
                if linked.trim().trim_start_matches("|->").trim() == input {
                    return true;
                }
            }
            return false;
        }
    }
    false
}

/// The `default` metadata entries for one subject (node id).
fn stream_metadata(node_id: u32) -> String {
    let out = std::process::Command::new("pw-metadata")
        .arg(node_id.to_string())
        .output()
        .expect("pw-metadata");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn pw_nodes() -> String {
    let out = std::process::Command::new("pw-cli")
        .args(["ls", "Node"])
        .output()
        .expect("pw-cli");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a running PipeWire; stop wavelinkd and Sonusmix first"]
async fn routes_moves_and_mixes_through_real_pipewire() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    let rec = dir.path().join("rec.wav");
    let silence = dir.path().join("silence.wav");
    support::write_tone_wav(&tone, 0.5, 3.0);
    support::write_tone_wav(&silence, 0.0, 10.0);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let backend = PipeWireBackend::spawn(tx);
    wait_for(&mut rx, "Ready", |e| {
        matches!(e, BackendEvent::Ready { .. })
    })
    .await;

    for (slot, id) in ["e2e-a", "e2e-b"].iter().enumerate() {
        backend.send(BackendCommand::EnsureChannel {
            id: id.to_string(),
            slot,
            description: format!("WaveLink {id}"),
            source: ChannelSource::Virtual,
        });
    }
    let mut targets = Targets::default();
    targets.channels[0] = Gains {
        monitor: 0.0,
        stream: 0.5,
    };
    backend.send(BackendCommand::SetTargets(targets));
    tokio::time::sleep(Duration::from_millis(500)).await;

    // A silent stream for the clear-target check below.
    let quiet = support::start_play(&silence, "wavelink.channel.e2e-a");
    let quiet_pid = quiet.id();
    let event = wait_for(&mut rx, "silent pw-play on channel a", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.app == "pw-play" && s.target.as_deref() == Some("wavelink.channel.e2e-a"))
    })
    .await;
    let BackendEvent::StreamUpdated(silent) = event else {
        unreachable!()
    };

    let recorder = support::start_record(&rec, "wavelink.stream_mix");
    let mut player = support::start_play(&tone, "wavelink.channel.e2e-b");
    let _reaper = Reaper(vec![quiet_pid, recorder.id(), player.id()]);
    let event = wait_for(&mut rx, "pw-play on channel b", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.app == "pw-play" && s.node_id != silent.node_id && s.target.as_deref() == Some("wavelink.channel.e2e-b"))
    })
    .await;
    let BackendEvent::StreamUpdated(stream) = event else {
        unreachable!()
    };

    // Deferred move to a channel that does not exist yet, superseded by a direct move:
    // when channel c appears later, the stale deferred move must not fire.
    backend.send(BackendCommand::MoveStream {
        node_id: stream.node_id,
        target: Some("wavelink.channel.e2e-c".into()),
    });
    backend.send(BackendCommand::MoveStream {
        node_id: stream.node_id,
        target: Some("wavelink.channel.e2e-a".into()),
    });
    wait_for(&mut rx, "move to channel a", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.node_id == stream.node_id && s.target.as_deref() == Some("wavelink.channel.e2e-a"))
    })
    .await;
    let meters = wait_for(
        &mut rx,
        "meters",
        |e| matches!(e, BackendEvent::Meters(p) if p.channels[0][0] > 0.1),
    )
    .await;
    assert!(matches!(meters, BackendEvent::Meters(_)));

    // A move with target None clears target.object again (silent stream, so nothing
    // becomes audible wherever it ends up).
    backend.send(BackendCommand::MoveStream {
        node_id: silent.node_id,
        target: Some("wavelink.channel.e2e-b".into()),
    });
    wait_for(&mut rx, "silent stream on channel b", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.node_id == silent.node_id && s.target.as_deref() == Some("wavelink.channel.e2e-b"))
    })
    .await;
    assert!(stream_metadata(silent.node_id).contains("target.object"));
    backend.send(BackendCommand::MoveStream {
        node_id: silent.node_id,
        target: None,
    });
    wait_for(&mut rx, "silent stream leaving channel b", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.node_id == silent.node_id && s.target.is_some() && s.target.as_deref() != Some("wavelink.channel.e2e-b"))
    })
    .await;
    let metadata = stream_metadata(silent.node_id);
    assert!(
        !metadata.contains("target.object"),
        "target.object not cleared:\n{metadata}"
    );

    // A link of ours destroyed externally gets recreated.
    support::pw_unlink("wavelink.mixer:stream_FL", "wavelink.stream_mix:input_FL");
    // Removing a channel destroys its node and its links while the loop runs; a
    // destroy must not provoke a core error that ends the session.
    backend.send(BackendCommand::RemoveChannel { id: "e2e-b".into() });
    backend.send(BackendCommand::EnsureChannel {
        id: "e2e-c".into(),
        slot: 2,
        description: "WaveLink e2e-c".into(),
        source: ChannelSource::Virtual,
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    let events = drain_checked(&mut rx);
    assert!(
        !events.iter().any(|e| matches!(e, BackendEvent::StreamUpdated(s) if s.node_id == stream.node_id && s.target.as_deref() == Some("wavelink.channel.e2e-c"))),
        "stale deferred move fired"
    );
    let nodes = pw_nodes();
    assert!(
        !nodes.contains("wavelink.channel.e2e-b"),
        "channel b still exists:\n{nodes}"
    );
    assert!(
        nodes.contains("wavelink.channel.e2e-a"),
        "channel a vanished:\n{nodes}"
    );
    assert!(
        nodes.contains("wavelink.channel.e2e-c"),
        "channel c missing:\n{nodes}"
    );
    assert!(
        has_link("wavelink.mixer:stream_FL", "wavelink.stream_mix:input_FL"),
        "removed link not recreated"
    );

    player.wait().unwrap();
    support::stop_record(quiet);
    support::stop_record(recorder);
    let peak = support::wav_peak(&rec);
    assert!((peak - 0.25).abs() < 0.03, "stream mix peak {peak}");

    backend.send(BackendCommand::Shutdown);
    assert!(backend.wait_stopped(Duration::from_secs(3)));
    drain_checked(&mut rx);
}
