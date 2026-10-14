//! Volume, mute, link, mix, listen, fallback and settings commands.
use serde_json::json;
use wavelink_core::model::{DeviceKind, MixKind, MixTarget};
use wavelink_core::protocol::ErrorBody;
use wavelink_core::resolve::{Candidate, resolve};
use wavelink_core::volume;

use super::{App, CmdResult, to_value};
use crate::backend::{AudioBackend, BackendCommand};

#[derive(Clone, Copy)]
enum LevelChange {
    Set(u8),
    Delta(i32),
}

impl LevelChange {
    fn apply(self, current: u8) -> u8 {
        match self {
            Self::Set(v) => v,
            Self::Delta(d) => volume::apply_delta(current, d),
        }
    }
}

fn level_change(value: Option<u8>, delta: Option<i32>) -> Result<LevelChange, ErrorBody> {
    match (value, delta) {
        (Some(v), None) if v <= 100 => Ok(LevelChange::Set(v)),
        (Some(v), None) => Err(ErrorBody::invalid(format!(
            "volume must be between 0 and 100, got {v}"
        ))),
        (None, Some(d)) => Ok(LevelChange::Delta(d)),
        (None, None) => Err(ErrorBody::invalid("either value or delta is required")),
        (Some(_), Some(_)) => Err(ErrorBody::invalid("value and delta are mutually exclusive")),
    }
}

fn other(kind: MixKind) -> MixKind {
    match kind {
        MixKind::Monitor => MixKind::Stream,
        MixKind::Stream => MixKind::Monitor,
    }
}

impl<B: AudioBackend> App<B> {
    pub(super) fn set_volume(
        &mut self,
        channel: &str,
        mix: MixTarget,
        value: Option<u8>,
        delta: Option<i32>,
    ) -> CmdResult {
        let change = level_change(value, delta)?;
        let id = self.resolve_channel(channel)?;
        let c = self
            .state
            .config
            .channel_mut(&id)
            .expect("resolved channel exists");
        match mix {
            MixTarget::Both => {
                for kind in [MixKind::Monitor, MixKind::Stream] {
                    let level = c.level_mut(kind);
                    level.volume = change.apply(level.volume);
                }
            }
            MixTarget::Monitor | MixTarget::Stream => {
                let kind = if mix == MixTarget::Monitor {
                    MixKind::Monitor
                } else {
                    MixKind::Stream
                };
                let old = c.level(kind).volume;
                let new = change.apply(old);
                c.level_mut(kind).volume = new;
                if c.linked {
                    let partner = c.level(other(kind)).volume;
                    c.level_mut(other(kind)).volume = volume::linked_partner(old, new, partner);
                }
            }
        }
        let result = to_value(&*c);
        self.push_targets();
        self.changed();
        Ok(result)
    }

    pub(super) fn set_mute(
        &mut self,
        channel: &str,
        mix: MixTarget,
        muted: Option<bool>,
    ) -> CmdResult {
        let id = self.resolve_channel(channel)?;
        let c = self
            .state
            .config
            .channel_mut(&id)
            .expect("resolved channel exists");
        let target = muted.unwrap_or_else(|| !mix.kinds().iter().all(|k| c.level(*k).muted));
        for kind in mix.kinds() {
            c.level_mut(*kind).muted = target;
        }
        let result = to_value(&*c);
        self.push_targets();
        self.changed();
        Ok(result)
    }

    pub(super) fn set_link(&mut self, channel: &str, linked: bool) -> CmdResult {
        let id = self.resolve_channel(channel)?;
        let c = self
            .state
            .config
            .channel_mut(&id)
            .expect("resolved channel exists");
        c.linked = linked;
        let result = to_value(&*c);
        self.changed();
        Ok(result)
    }

    pub(super) fn set_mix(
        &mut self,
        mix: MixKind,
        volume: Option<u8>,
        delta: Option<i32>,
        muted: Option<bool>,
        output_device: Option<String>,
    ) -> CmdResult {
        let change = match (volume, delta) {
            (None, None) => None,
            (v, d) => Some(level_change(v, d)?),
        };
        let device = match output_device {
            None => None,
            Some(_) if mix == MixKind::Stream => {
                return Err(ErrorBody::invalid("the stream mix has no output device"));
            }
            Some(query) => Some(self.resolve_sink(&query)?),
        };
        if change.is_none() && muted.is_none() && device.is_none() {
            return Err(ErrorBody::invalid(
                "nothing to change: give volume, delta, muted or output_device",
            ));
        }
        let m = self.state.config.mixes.get_mut(mix);
        if let Some(change) = change {
            m.volume = change.apply(m.volume);
        }
        if let Some(muted) = muted {
            m.muted = muted;
        }
        if let Some(device) = &device {
            m.output_device = Some(device.clone());
        }
        let result = to_value(&*m);
        if let Some(device) = device {
            self.backend
                .send(BackendCommand::SetMonitorOutput(Some(device)));
        }
        self.push_targets();
        self.changed();
        Ok(result)
    }

    pub(super) fn set_listen(&mut self, mix: MixKind) -> CmdResult {
        self.state.config.listen = mix;
        self.push_targets();
        self.changed();
        Ok(json!({ "listen": mix }))
    }

    pub(super) fn set_fallback_channel(&mut self, channel: &str) -> CmdResult {
        let id = self.resolve_channel(channel)?;
        self.state.config.fallback_channel = id.clone();
        self.backend
            .send(BackendCommand::SetDefaultSink(Some(self.fallback_node())));
        self.changed();
        Ok(json!({ "fallback_channel": id }))
    }

    pub(super) fn set_settings(&mut self, auto_xlr_profile: Option<bool>) -> CmdResult {
        if let Some(value) = auto_xlr_profile {
            self.state.config.settings.auto_xlr_profile = value;
        }
        self.changed();
        Ok(to_value(&self.state.config.settings))
    }

    fn resolve_sink(&self, query: &str) -> Result<String, ErrorBody> {
        let candidates: Vec<Candidate> = self
            .state
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Sink)
            .map(|d| Candidate {
                id: &d.name,
                name: &d.description,
            })
            .collect();
        resolve(query, &candidates)
            .map(str::to_owned)
            .map_err(ErrorBody::from)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use wavelink_core::model::{DeviceKind, MixKind, MixTarget};
    use wavelink_core::protocol::{Command, ErrorCode};

    use crate::backend::{BackendCommand, BackendEvent};

    fn vol(channel: &str, mix: MixTarget, value: Option<u8>, delta: Option<i32>) -> Command {
        Command::SetVolume {
            channel: channel.into(),
            mix,
            value,
            delta,
        }
    }

    #[test]
    fn set_volume_updates_level_targets_and_dirty_flags() {
        let (mut app, fake) = ready_app();
        let result = app
            .handle(vol("music", MixTarget::Stream, Some(50), None))
            .unwrap();
        assert_eq!(result["stream"]["volume"], 50);
        assert!((last_targets(&fake.take()).channels[1].stream - 0.125).abs() < 1e-6);
        assert!(app.take_config_dirty());
        assert!(app.take_state_dirty());
    }

    #[test]
    fn delta_is_clamped_and_fuzzy_names_work() {
        let (mut app, _fake) = ready_app();
        app.handle(vol("voice", MixTarget::Monitor, None, Some(30)))
            .unwrap();
        assert_eq!(
            app.state()
                .config
                .channel("voice-chat")
                .unwrap()
                .monitor
                .volume,
            100
        );
    }

    #[test]
    fn invalid_volume_arguments_are_rejected() {
        let (mut app, _fake) = ready_app();
        for cmd in [
            vol("music", MixTarget::Stream, Some(150), None),
            vol("music", MixTarget::Stream, None, None),
            vol("music", MixTarget::Stream, Some(10), Some(5)),
        ] {
            assert_eq!(
                app.handle(cmd).unwrap_err().code,
                ErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn unknown_channel_lists_options() {
        let (mut app, _fake) = ready_app();
        let err = app
            .handle(vol("xyz", MixTarget::Stream, Some(10), None))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.options, vec!["System", "Music", "Voice Chat"]);
    }

    #[test]
    fn linked_faders_keep_their_distance() {
        let (mut app, _fake) = ready_app();
        app.handle(vol("music", MixTarget::Stream, Some(80), None))
            .unwrap();
        app.handle(vol("music", MixTarget::Monitor, Some(50), None))
            .unwrap();
        app.handle(Command::SetLink {
            channel: "music".into(),
            linked: true,
        })
        .unwrap();
        app.handle(vol("music", MixTarget::Monitor, Some(40), None))
            .unwrap();
        let music = app.state().config.channel("music").unwrap();
        assert_eq!((music.monitor.volume, music.stream.volume), (40, 70));
    }

    #[test]
    fn both_sets_both_faders() {
        let (mut app, _fake) = ready_app();
        app.handle(vol("music", MixTarget::Both, Some(30), None))
            .unwrap();
        let music = app.state().config.channel("music").unwrap();
        assert_eq!((music.monitor.volume, music.stream.volume), (30, 30));
    }

    #[test]
    fn mute_toggle_mutes_both_unless_all_muted() {
        let (mut app, fake) = ready_app();
        let mute = |muted| Command::SetMute {
            channel: "music".into(),
            mix: MixTarget::Both,
            muted,
        };
        app.handle(Command::SetMute {
            channel: "music".into(),
            mix: MixTarget::Stream,
            muted: Some(true),
        })
        .unwrap();
        app.handle(mute(None)).unwrap();
        let music = app.state().config.channel("music").unwrap();
        assert!(music.monitor.muted && music.stream.muted);
        app.handle(mute(None)).unwrap();
        let music = app.state().config.channel("music").unwrap();
        assert!(!music.monitor.muted && !music.stream.muted);
        assert_eq!(last_targets(&fake.take()).channels[1].stream, 1.0);
    }

    #[test]
    fn set_mix_changes_volume_mute_and_device() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            "alsa_output.hdmi",
            "HDMI Speakers",
            DeviceKind::Sink,
        )));
        let result = app
            .handle(Command::SetMix {
                mix: MixKind::Monitor,
                volume: Some(50),
                delta: None,
                muted: None,
                output_device: Some("hdmi".into()),
            })
            .unwrap();
        assert_eq!(result["output_device"], "alsa_output.hdmi");
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::SetMonitorOutput(Some(
            "alsa_output.hdmi".into()
        ))));
        assert!((last_targets(&cmds).monitor_master - 0.125).abs() < 1e-6);
    }

    #[test]
    fn stream_mix_has_no_output_device_and_empty_set_mix_is_invalid() {
        let (mut app, _fake) = ready_app();
        let err = app
            .handle(Command::SetMix {
                mix: MixKind::Stream,
                volume: None,
                delta: None,
                muted: None,
                output_device: Some("x".into()),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        let err = app
            .handle(Command::SetMix {
                mix: MixKind::Stream,
                volume: None,
                delta: None,
                muted: None,
                output_device: None,
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn listen_switch_reaches_targets() {
        let (mut app, fake) = ready_app();
        app.handle(Command::SetListen {
            mix: MixKind::Stream,
        })
        .unwrap();
        assert!(last_targets(&fake.take()).listen_stream);
    }

    #[test]
    fn fallback_change_sets_default_sink() {
        let (mut app, fake) = ready_app();
        app.handle(Command::SetFallbackChannel {
            channel: "music".into(),
        })
        .unwrap();
        assert_eq!(app.state().config.fallback_channel, "music");
        assert!(fake.take().contains(&BackendCommand::SetDefaultSink(Some(
            "wavelink.channel.music".into()
        ))));
    }

    #[test]
    fn settings_are_updated() {
        let (mut app, _fake) = ready_app();
        app.handle(Command::SetSettings {
            auto_xlr_profile: Some(false),
        })
        .unwrap();
        assert!(!app.state().config.settings.auto_xlr_profile);
    }
}
