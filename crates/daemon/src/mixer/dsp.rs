//! Pure mixing function: no allocation, no locks – safe for the real-time thread.
use mixlink_core::model::MAX_CHANNELS;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Gains {
    pub monitor: f32,
    pub stream: f32,
}

/// Desired linear gains; mute is already folded in (gain 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Targets {
    pub channels: [Gains; MAX_CHANNELS],
    pub monitor_master: f32,
    pub stream_master: f32,
    /// When true, the monitor output carries the stream mix.
    pub listen_stream: bool,
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            channels: [Gains::default(); MAX_CHANNELS],
            monitor_master: 1.0,
            stream_master: 1.0,
            listen_stream: false,
        }
    }
}

/// Current (ramping) gains; starts silent so the mixer fades in.
#[derive(Debug, Clone, Default)]
pub struct DspState {
    channels: [Gains; MAX_CHANNELS],
    monitor_master: f32,
    stream_master: f32,
    listen: f32,
}

impl DspState {
    /// A state that already sits on `targets` (no ramp).
    pub fn settled(targets: &Targets) -> Self {
        Self {
            channels: targets.channels,
            monitor_master: targets.monitor_master,
            stream_master: targets.stream_master,
            listen: if targets.listen_stream { 1.0 } else { 0.0 },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Peaks {
    /// Per slot, before the faders.
    pub channels: [[f32; 2]; MAX_CHANNELS],
    /// Monitor output after master.
    pub monitor: [f32; 2],
    /// Stream output after master.
    pub stream: [f32; 2],
}

pub struct Block<'a> {
    pub inputs: [[Option<&'a [f32]>; 2]; MAX_CHANNELS],
    pub monitor: [&'a mut [f32]; 2],
    pub stream: [&'a mut [f32]; 2],
}

/// Maximum gain change per sample so that 0 → 1 takes 10 ms.
pub fn ramp_step(sample_rate: u32) -> f32 {
    1.0 / (0.010 * sample_rate.max(1) as f32)
}

fn approach(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else if target > current {
        current + step
    } else {
        current - step
    }
}

pub fn process(
    state: &mut DspState,
    targets: &Targets,
    block: Block<'_>,
    step: f32,
    peaks: &mut Peaks,
) {
    let Block {
        inputs,
        monitor: [mon_l, mon_r],
        stream: [str_l, str_r],
    } = block;
    let n = mon_l
        .len()
        .min(mon_r.len())
        .min(str_l.len())
        .min(str_r.len());
    let listen_target = if targets.listen_stream { 1.0 } else { 0.0 };
    for i in 0..n {
        let (mut ml, mut mr, mut sl, mut sr) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for (slot, input) in inputs.iter().enumerate() {
            let gains = &mut state.channels[slot];
            let target = targets.channels[slot];
            gains.monitor = approach(gains.monitor, target.monitor, step);
            gains.stream = approach(gains.stream, target.stream, step);
            let l = input[0].and_then(|b| b.get(i)).copied().unwrap_or(0.0);
            let r = input[1].and_then(|b| b.get(i)).copied().unwrap_or(0.0);
            let peak = &mut peaks.channels[slot];
            peak[0] = peak[0].max(l.abs());
            peak[1] = peak[1].max(r.abs());
            ml += l * gains.monitor;
            mr += r * gains.monitor;
            sl += l * gains.stream;
            sr += r * gains.stream;
        }
        state.monitor_master = approach(state.monitor_master, targets.monitor_master, step);
        state.stream_master = approach(state.stream_master, targets.stream_master, step);
        state.listen = approach(state.listen, listen_target, step);

        let (sl, sr) = (sl * state.stream_master, sr * state.stream_master);
        let x = state.listen;
        let ol = (ml * (1.0 - x) + sl * x) * state.monitor_master;
        let or = (mr * (1.0 - x) + sr * x) * state.monitor_master;

        mon_l[i] = ol;
        mon_r[i] = or;
        str_l[i] = sl;
        str_r[i] = sr;
        peaks.monitor[0] = peaks.monitor[0].max(ol.abs());
        peaks.monitor[1] = peaks.monitor[1].max(or.abs());
        peaks.stream[0] = peaks.stream[0].max(sl.abs());
        peaks.stream[1] = peaks.stream[1].max(sr.abs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Out = ([Vec<f32>; 2], [Vec<f32>; 2], Peaks);

    fn run(
        state: &mut DspState,
        targets: &Targets,
        inputs: &[(usize, f32)],
        n: usize,
        step: f32,
    ) -> Out {
        let bufs: Vec<(usize, Vec<f32>)> =
            inputs.iter().map(|&(slot, v)| (slot, vec![v; n])).collect();
        let mut ins: [[Option<&[f32]>; 2]; MAX_CHANNELS] = [[None; 2]; MAX_CHANNELS];
        for (slot, b) in &bufs {
            ins[*slot] = [Some(b.as_slice()), Some(b.as_slice())];
        }
        let (mut ml, mut mr, mut sl, mut sr) =
            (vec![9.0; n], vec![9.0; n], vec![9.0; n], vec![9.0; n]);
        let mut peaks = Peaks::default();
        process(
            state,
            targets,
            Block {
                inputs: ins,
                monitor: [&mut ml[..], &mut mr[..]],
                stream: [&mut sl[..], &mut sr[..]],
            },
            step,
            &mut peaks,
        );
        ([ml, mr], [sl, sr], peaks)
    }

    fn targets(gains: &[(usize, f32, f32)]) -> Targets {
        let mut t = Targets::default();
        for &(slot, monitor, stream) in gains {
            t.channels[slot] = Gains { monitor, stream };
        }
        t
    }

    #[test]
    fn unity_gain_passes_signal_to_both_mixes() {
        let t = targets(&[(0, 1.0, 1.0)]);
        let (mon, st, _) = run(&mut DspState::settled(&t), &t, &[(0, 0.5)], 8, 0.1);
        assert!(
            mon[0]
                .iter()
                .chain(&mon[1])
                .chain(&st[0])
                .chain(&st[1])
                .all(|&s| (s - 0.5).abs() < 1e-6)
        );
    }

    #[test]
    fn channels_are_summed() {
        let t = targets(&[(0, 1.0, 1.0), (1, 1.0, 1.0)]);
        let (mon, _, _) = run(
            &mut DspState::settled(&t),
            &t,
            &[(0, 0.25), (1, 0.5)],
            4,
            0.1,
        );
        assert!((mon[0][3] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn muted_stream_path_is_silent_but_channel_peak_is_pre_fader() {
        let t = targets(&[(0, 1.0, 0.0)]);
        let (mon, st, peaks) = run(&mut DspState::settled(&t), &t, &[(0, 0.5)], 4, 0.1);
        assert!(st[0].iter().all(|&s| s == 0.0));
        assert!((mon[0][0] - 0.5).abs() < 1e-6);
        assert_eq!(peaks.channels[0], [0.5, 0.5]);
        assert_eq!(peaks.stream, [0.0, 0.0]);
        assert!((peaks.monitor[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn gain_changes_are_ramped() {
        let t = targets(&[(0, 1.0, 1.0)]);
        // Masters already at 1.0, channel gains start at 0 and must ramp.
        let mut state = DspState::settled(&Targets::default());
        let (_, st, _) = run(&mut state, &t, &[(0, 1.0)], 20, 0.1);
        assert!((st[0][0] - 0.1).abs() < 1e-6);
        assert!((st[0][9] - 1.0).abs() < 1e-6);
        assert!(st[0].windows(2).all(|w| w[1] - w[0] <= 0.1 + 1e-6));
    }

    #[test]
    fn listening_to_stream_puts_stream_mix_on_monitor_output() {
        let mut t = targets(&[(0, 0.0, 1.0)]);
        t.listen_stream = true;
        t.monitor_master = 0.5;
        let (mon, st, _) = run(&mut DspState::settled(&t), &t, &[(0, 0.5)], 4, 0.1);
        assert!((st[0][0] - 0.5).abs() < 1e-6);
        assert!((mon[0][0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn stream_master_scales_only_the_stream() {
        let mut t = targets(&[(0, 1.0, 1.0)]);
        t.stream_master = 0.5;
        let (mon, st, _) = run(&mut DspState::settled(&t), &t, &[(0, 0.5)], 4, 0.1);
        assert!((st[0][0] - 0.25).abs() < 1e-6);
        assert!((mon[0][0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn missing_inputs_are_silence() {
        let t = targets(&[(0, 1.0, 1.0)]);
        let (mon, st, _) = run(&mut DspState::settled(&t), &t, &[], 4, 0.1);
        assert!(mon[0].iter().chain(&st[1]).all(|&s| s == 0.0));
    }

    #[test]
    fn ramp_step_covers_full_scale_in_ten_ms() {
        assert!((ramp_step(48_000) - 1.0 / 480.0).abs() < 1e-9);
    }
}
