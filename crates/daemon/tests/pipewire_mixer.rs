mod support;

use std::sync::Arc;
use std::time::Duration;

use pipewire as pw;
use wavelinkd::mixer::dsp::{Gains, Targets};
use wavelinkd::mixer::shared::SharedParams;
use wavelinkd::pw::mixer_node::MixerNode;
use wavelinkd::pw::nodes::create_null_sink;

#[test]
#[ignore = "needs a running PipeWire"]
fn mixer_node_applies_stream_gain_and_reports_peaks() {
    pw::init();
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    let rec = dir.path().join("rec.wav");
    support::write_tone_wav(&tone, 0.5, 1.0);

    let shared = Arc::new(SharedParams::new());
    let mut targets = Targets::default();
    targets.channels[0] = Gains { monitor: 0.0, stream: 0.5 };
    shared.store_targets(&targets);

    let (done_tx, done_rx) = pw::channel::channel::<()>();
    let (tone_w, rec_w) = (tone.clone(), rec.clone());
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        for ch in ["FL", "FR"] {
            support::pw_link(&format!("wltest.ch:monitor_{ch}"), &format!("wltest.mixer:in_01_{ch}"));
            support::pw_link(&format!("wltest.mixer:stream_{ch}"), &format!("wltest.out:input_{ch}"));
        }
        let recorder = support::start_record(&rec_w, "wltest.out");
        std::thread::sleep(Duration::from_millis(300));
        support::start_play(&tone_w, "wltest.ch").wait().unwrap();
        support::stop_record(recorder);
        done_tx.send(()).unwrap();
    });

    let mainloop = pw::main_loop::MainLoopRc::new(None).unwrap();
    let context = pw::context::ContextRc::new(&mainloop, None).unwrap();
    let core = context.connect_rc(None).unwrap();
    let mixer = MixerNode::new(&core, "wltest.mixer", shared.clone()).unwrap();
    let channel = create_null_sink(&core, "wltest.ch", "WL Test Channel", "Audio/Sink").unwrap();
    let out = create_null_sink(&core, "wltest.out", "WL Test Out", "Audio/Source/Virtual").unwrap();
    let quit = mainloop.clone();
    let _rx = done_rx.attach(mainloop.loop_(), move |_| quit.quit());
    mainloop.run();
    worker.join().unwrap();

    let peaks = shared.take_peaks();
    drop((mixer, channel, out));
    let peak = support::wav_peak(&rec);
    assert!((peak - 0.25).abs() < 0.02, "recorded peak {peak}");
    // Nothing took the peaks during playback, so they hold the maxima of the whole run.
    assert!((peaks.channels[0][0] - 0.5).abs() < 0.05, "pre-fader channel peak {:?}", peaks.channels[0]);
    assert!((peaks.stream[0] - 0.25).abs() < 0.05, "stream peak {:?}", peaks.stream);
}
