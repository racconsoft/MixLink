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
            if pred(&event) {
                return event;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a running PipeWire; stop wavelinkd and Sonusmix first"]
async fn routes_moves_and_mixes_through_real_pipewire() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    let rec = dir.path().join("rec.wav");
    support::write_tone_wav(&tone, 0.5, 3.0);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let backend = PipeWireBackend::spawn(tx);
    wait_for(&mut rx, "Ready", |e| matches!(e, BackendEvent::Ready { .. })).await;

    for (slot, id) in ["e2e-a", "e2e-b"].iter().enumerate() {
        backend.send(BackendCommand::EnsureChannel {
            id: id.to_string(),
            slot,
            description: format!("WaveLink {id}"),
            source: ChannelSource::Virtual,
        });
    }
    let mut targets = Targets::default();
    targets.channels[0] = Gains { monitor: 0.0, stream: 0.5 };
    backend.send(BackendCommand::SetTargets(targets));
    tokio::time::sleep(Duration::from_millis(500)).await;

    let recorder = support::start_record(&rec, "wavelink.stream_mix");
    let mut player = support::start_play(&tone, "wavelink.channel.e2e-b");
    let event = wait_for(&mut rx, "pw-play on channel b", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.app == "pw-play" && s.target.as_deref() == Some("wavelink.channel.e2e-b"))
    })
    .await;
    let BackendEvent::StreamUpdated(stream) = event else { unreachable!() };

    backend.send(BackendCommand::MoveStream { node_id: stream.node_id, target: "wavelink.channel.e2e-a".into() });
    wait_for(&mut rx, "move to channel a", |e| {
        matches!(e, BackendEvent::StreamUpdated(s) if s.node_id == stream.node_id && s.target.as_deref() == Some("wavelink.channel.e2e-a"))
    })
    .await;
    let meters = wait_for(&mut rx, "meters", |e| matches!(e, BackendEvent::Meters(p) if p.channels[0][0] > 0.1)).await;
    assert!(matches!(meters, BackendEvent::Meters(_)));

    player.wait().unwrap();
    support::stop_record(recorder);
    let peak = support::wav_peak(&rec);
    assert!((peak - 0.25).abs() < 0.03, "stream mix peak {peak}");

    backend.send(BackendCommand::Shutdown);
    assert!(backend.wait_stopped(Duration::from_secs(3)));
}
