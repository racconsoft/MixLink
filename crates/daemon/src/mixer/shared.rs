//! Lock-free exchange of targets (control → RT) and peaks (RT → control).
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use mixlink_core::model::MAX_CHANNELS;

use crate::mixer::dsp::{Gains, Peaks, Targets};

struct AtomicF32(AtomicU32);

impl AtomicF32 {
    fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }
    fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Relaxed))
    }
    fn store(&self, value: f32) {
        self.0.store(value.to_bits(), Relaxed);
    }
    /// Only valid for non-negative values, whose bit patterns order like the floats.
    fn fetch_max(&self, value: f32) {
        self.0.fetch_max(value.max(0.0).to_bits(), Relaxed);
    }
    fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0, Relaxed))
    }
}

fn pairs() -> [[AtomicF32; 2]; MAX_CHANNELS] {
    std::array::from_fn(|_| [AtomicF32::new(0.0), AtomicF32::new(0.0)])
}

pub struct SharedParams {
    monitor: [AtomicF32; MAX_CHANNELS],
    stream: [AtomicF32; MAX_CHANNELS],
    monitor_master: AtomicF32,
    stream_master: AtomicF32,
    listen_stream: AtomicBool,
    peak_channels: [[AtomicF32; 2]; MAX_CHANNELS],
    peak_monitor: [AtomicF32; 2],
    peak_stream: [AtomicF32; 2],
}

impl Default for SharedParams {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedParams {
    pub fn new() -> Self {
        Self {
            monitor: std::array::from_fn(|_| AtomicF32::new(0.0)),
            stream: std::array::from_fn(|_| AtomicF32::new(0.0)),
            monitor_master: AtomicF32::new(1.0),
            stream_master: AtomicF32::new(1.0),
            listen_stream: AtomicBool::new(false),
            peak_channels: pairs(),
            peak_monitor: [AtomicF32::new(0.0), AtomicF32::new(0.0)],
            peak_stream: [AtomicF32::new(0.0), AtomicF32::new(0.0)],
        }
    }

    pub fn store_targets(&self, t: &Targets) {
        for (slot, gains) in t.channels.iter().enumerate() {
            self.monitor[slot].store(gains.monitor);
            self.stream[slot].store(gains.stream);
        }
        self.monitor_master.store(t.monitor_master);
        self.stream_master.store(t.stream_master);
        self.listen_stream.store(t.listen_stream, Relaxed);
    }

    pub fn load_targets(&self) -> Targets {
        let mut t = Targets::default();
        for slot in 0..MAX_CHANNELS {
            t.channels[slot] = Gains {
                monitor: self.monitor[slot].load(),
                stream: self.stream[slot].load(),
            };
        }
        t.monitor_master = self.monitor_master.load();
        t.stream_master = self.stream_master.load();
        t.listen_stream = self.listen_stream.load(Relaxed);
        t
    }

    pub fn publish_peaks(&self, p: &Peaks) {
        for (slot, pair) in p.channels.iter().enumerate() {
            self.peak_channels[slot][0].fetch_max(pair[0]);
            self.peak_channels[slot][1].fetch_max(pair[1]);
        }
        for c in 0..2 {
            self.peak_monitor[c].fetch_max(p.monitor[c]);
            self.peak_stream[c].fetch_max(p.stream[c]);
        }
    }

    /// Returns the maxima since the last call and resets them.
    pub fn take_peaks(&self) -> Peaks {
        let mut p = Peaks::default();
        for slot in 0..MAX_CHANNELS {
            p.channels[slot] = [
                self.peak_channels[slot][0].take(),
                self.peak_channels[slot][1].take(),
            ];
        }
        p.monitor = [self.peak_monitor[0].take(), self.peak_monitor[1].take()];
        p.stream = [self.peak_stream[0].take(), self.peak_stream[1].take()];
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::dsp::Gains;

    #[test]
    fn targets_roundtrip() {
        let shared = SharedParams::new();
        let mut t = Targets::default();
        t.channels[3] = Gains {
            monitor: 0.25,
            stream: 0.5,
        };
        t.stream_master = 0.75;
        t.listen_stream = true;
        shared.store_targets(&t);
        assert_eq!(shared.load_targets(), t);
    }

    #[test]
    fn peaks_keep_maximum_until_taken() {
        let shared = SharedParams::new();
        let mut p = Peaks::default();
        p.channels[0] = [0.5, 0.25];
        p.stream = [0.3, 0.3];
        shared.publish_peaks(&p);
        p.channels[0] = [0.1, 0.9];
        shared.publish_peaks(&p);
        let taken = shared.take_peaks();
        assert_eq!(taken.channels[0], [0.5, 0.9]);
        assert_eq!(taken.stream, [0.3, 0.3]);
        assert_eq!(shared.take_peaks(), Peaks::default());
    }

    #[test]
    fn is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SharedParams>();
    }
}
