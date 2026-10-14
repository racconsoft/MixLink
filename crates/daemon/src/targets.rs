//! Translates the configuration into mixer targets.
use wavelink_core::model::{ChannelId, Config, MAX_CHANNELS, MixKind};
use wavelink_core::volume::to_gain;

use crate::mixer::dsp::{Gains, Targets};

pub fn compute_targets(config: &Config, slots: &[Option<ChannelId>; MAX_CHANNELS]) -> Targets {
    let mut t = Targets::default();
    for (slot, id) in slots.iter().enumerate() {
        let Some(channel) = id.as_deref().and_then(|id| config.channel(id)) else {
            continue;
        };
        t.channels[slot] = Gains {
            monitor: level_gain(channel.monitor.volume, channel.monitor.muted),
            stream: level_gain(channel.stream.volume, channel.stream.muted),
        };
    }
    t.monitor_master = level_gain(config.mixes.monitor.volume, config.mixes.monitor.muted);
    t.stream_master = level_gain(config.mixes.stream.volume, config.mixes.stream.muted);
    t.listen_stream = config.listen == MixKind::Stream;
    t
}

fn level_gain(volume: u8, muted: bool) -> f32 {
    if muted { 0.0 } else { to_gain(volume) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wavelink_core::model::Channel;

    fn config() -> Config {
        let mut music = Channel::new("music", "Music");
        music.stream.volume = 50;
        music.monitor.muted = true;
        Config {
            version: 1,
            fallback_channel: "system".into(),
            listen: MixKind::Stream,
            settings: Default::default(),
            mixes: Default::default(),
            channels: vec![Channel::new("system", "System"), music],
        }
    }

    fn slots() -> [Option<ChannelId>; MAX_CHANNELS] {
        let mut s: [Option<ChannelId>; MAX_CHANNELS] = Default::default();
        s[0] = Some("system".into());
        s[5] = Some("music".into());
        s
    }

    #[test]
    fn maps_channel_levels_to_slot_gains() {
        let t = compute_targets(&config(), &slots());
        assert_eq!(t.channels[0].monitor, 1.0);
        assert!((t.channels[5].stream - 0.125).abs() < 1e-6);
        assert_eq!(t.channels[5].monitor, 0.0, "muted → 0");
        assert_eq!(t.channels[1].stream, 0.0, "empty slot → 0");
        assert!(t.listen_stream);
    }

    #[test]
    fn mix_mute_zeroes_master() {
        let mut c = config();
        c.mixes.stream.muted = true;
        c.mixes.monitor.volume = 50;
        let t = compute_targets(&c, &slots());
        assert_eq!(t.stream_master, 0.0);
        assert!((t.monitor_master - 0.125).abs() < 1e-6);
    }

    #[test]
    fn slot_pointing_to_unknown_channel_is_silent() {
        let mut s = slots();
        s[2] = Some("ghost".into());
        assert_eq!(compute_targets(&config(), &s).channels[2].monitor, 0.0);
    }
}
