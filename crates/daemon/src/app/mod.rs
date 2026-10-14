//! Central state machine: owns the State, applies commands, drives the backend.
use mixlink_core::model::{
    ActiveApp, Channel, ChannelId, ChannelKind, Config, Device, DeviceKind, MAX_CHANNELS,
    NODE_PREFIX, State, channel_id_from_node_name, channel_node_name,
};
use mixlink_core::protocol::{Command, ErrorBody, Event};
use mixlink_core::resolve::{Candidate, resolve};
use mixlink_core::rules;
use serde::Serialize;
use serde_json::Value;

use crate::backend::{AudioBackend, BackendCommand, BackendEvent, ChannelSource, StreamInfo};
use crate::mixer::dsp::Peaks;
use crate::targets::compute_targets;

mod channels;
mod mixing;

pub type CmdResult = Result<Value, ErrorBody>;

const XLR_MARKER: &str = "Elgato_Wave_XLR";

pub struct App<B: AudioBackend> {
    state: State,
    backend: B,
    slots: [Option<ChannelId>; MAX_CHANNELS],
    previous_default_sink: Option<String>,
    first_run: bool,
    config_dirty: bool,
    state_dirty: bool,
}

impl<B: AudioBackend> App<B> {
    pub fn new(mut config: Config, backend: B, first_run: bool) -> Self {
        let mut warnings = Vec::new();
        if config.channels.len() > MAX_CHANNELS {
            warnings.push(format!(
                "Only {MAX_CHANNELS} channels are supported; the rest were ignored"
            ));
            config.channels.truncate(MAX_CHANNELS);
        }
        let mut slots: [Option<ChannelId>; MAX_CHANNELS] = Default::default();
        for (slot, channel) in slots.iter_mut().zip(&config.channels) {
            *slot = Some(channel.id.clone());
        }
        Self {
            state: State {
                config,
                apps: Vec::new(),
                devices: Vec::new(),
                warnings,
            },
            backend,
            slots,
            previous_default_sink: None,
            first_run,
            config_dirty: first_run,
            state_dirty: false,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.state.warnings.push(warning.into());
        self.state_dirty = true;
    }

    pub fn take_config_dirty(&mut self) -> bool {
        std::mem::take(&mut self.config_dirty)
    }

    pub fn take_state_dirty(&mut self) -> bool {
        std::mem::take(&mut self.state_dirty)
    }

    pub fn state_event(&self) -> Event {
        Event::StateChanged {
            state: self.state.clone(),
        }
    }

    pub fn handle(&mut self, cmd: Command) -> CmdResult {
        match cmd {
            Command::GetState => Ok(to_value(&self.state)),
            Command::Subscribe { .. } => Ok(Value::Null),
            Command::ListApps => Ok(to_value(&self.state.apps)),
            Command::ListDevices => Ok(to_value(&self.state.devices)),
            Command::SetVolume {
                channel,
                mix,
                value,
                delta,
            } => self.set_volume(&channel, mix, value, delta),
            Command::SetMute {
                channel,
                mix,
                muted,
            } => self.set_mute(&channel, mix, muted),
            Command::SetLink { channel, linked } => self.set_link(&channel, linked),
            Command::SetMix {
                mix,
                volume,
                delta,
                muted,
                output_device,
            } => self.set_mix(mix, volume, delta, muted, output_device),
            Command::SetListen { mix } => self.set_listen(mix),
            Command::SetFallbackChannel { channel } => self.set_fallback_channel(&channel),
            Command::SetSettings { auto_xlr_profile } => self.set_settings(auto_xlr_profile),
            Command::AssignApp { app, channel } => self.assign_app(&app, &channel),
            Command::UnassignApp { app } => self.unassign_app(&app),
            Command::CreateChannel { name, icon, color } => self.create_channel(&name, icon, color),
            Command::RenameChannel { channel, name } => self.rename_channel(&channel, &name),
            Command::UpdateChannel {
                channel,
                icon,
                color,
            } => self.update_channel(&channel, icon, color),
            Command::DeleteChannel { channel } => self.delete_channel(&channel),
            Command::ReorderChannels { order } => self.reorder_channels(&order),
        }
    }

    pub fn on_backend_event(&mut self, event: BackendEvent) -> Option<Event> {
        match event {
            BackendEvent::Ready {
                previous_default_sink,
            } => {
                if let Some(prev) = previous_default_sink.filter(|p| !p.starts_with(NODE_PREFIX)) {
                    self.previous_default_sink = Some(prev);
                }
                if self.state.config.mixes.monitor.output_device.is_none() {
                    let fallback = self.previous_default_sink.clone().or_else(|| {
                        self.state
                            .devices
                            .iter()
                            .find(|d| d.kind == DeviceKind::Sink)
                            .map(|d| d.name.clone())
                    });
                    if let Some(device) = fallback {
                        self.state.config.mixes.monitor.output_device = Some(device);
                        self.changed();
                    }
                }
                self.resync();
                // First-run magic only applies to the devices present at startup.
                self.first_run = false;
                None
            }
            BackendEvent::StreamUpdated(info) => {
                self.on_stream(info);
                None
            }
            BackendEvent::StreamRemoved(node_id) => {
                self.state.apps.retain(|a| a.node_id != node_id);
                self.state_dirty = true;
                None
            }
            BackendEvent::DeviceAdded(device) => {
                self.on_device_added(device);
                None
            }
            BackendEvent::DeviceRemoved(name) => {
                self.state.devices.retain(|d| d.name != name);
                self.state_dirty = true;
                None
            }
            BackendEvent::Meters(peaks) => Some(self.meters_event(&peaks)),
            BackendEvent::Disconnected => {
                self.state.apps.clear();
                self.state.devices.clear();
                self.state_dirty = true;
                None
            }
        }
    }

    /// Restores the previous default sink and stops the backend.
    pub fn shutdown(&mut self) {
        self.backend.send(BackendCommand::SetDefaultSink(
            self.previous_default_sink.clone(),
        ));
        self.backend.send(BackendCommand::Shutdown);
    }

    fn meters_event(&self, peaks: &Peaks) -> Event {
        let channels = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, id)| id.as_ref().map(|id| (id.clone(), peaks.channels[slot])))
            .collect();
        Event::Meters {
            channels,
            monitor: peaks.monitor,
            stream: peaks.stream,
        }
    }

    fn on_stream(&mut self, info: StreamInfo) {
        let channel = info
            .target
            .as_deref()
            .and_then(channel_id_from_node_name)
            .map(str::to_owned);
        let entry = ActiveApp {
            node_id: info.node_id,
            app: info.app.clone(),
            name: info.name,
            icon: info.icon,
            channel: channel.clone(),
        };
        let is_new = match self
            .state
            .apps
            .iter_mut()
            .find(|a| a.node_id == info.node_id)
        {
            Some(existing) => {
                *existing = entry;
                false
            }
            None => {
                self.state.apps.push(entry);
                true
            }
        };
        self.state_dirty = true;
        if !is_new {
            return;
        }
        if let Some(target) = rules::channel_for_app(&self.state.config.channels, &info.app)
            && channel.as_deref() != Some(target.id.as_str())
        {
            self.backend.send(BackendCommand::MoveStream {
                node_id: info.node_id,
                target: Some(target.node_name()),
            });
        }
    }

    fn on_device_added(&mut self, device: Device) {
        if self.first_run && device.name.contains(XLR_MARKER) {
            match device.kind {
                DeviceKind::Source
                    if !self
                        .state
                        .config
                        .channels
                        .iter()
                        .any(|c| c.kind == ChannelKind::Input) =>
                {
                    self.add_mic_channel(&device.name);
                }
                DeviceKind::Sink
                    if self.state.config.mixes.monitor.output_device.as_deref()
                        != Some(&device.name) =>
                {
                    self.state.config.mixes.monitor.output_device = Some(device.name.clone());
                    self.backend
                        .send(BackendCommand::SetMonitorOutput(Some(device.name.clone())));
                    self.changed();
                }
                _ => {}
            }
        }
        match self
            .state
            .devices
            .iter_mut()
            .find(|d| d.name == device.name)
        {
            Some(existing) => *existing = device,
            None => self.state.devices.push(device),
        }
        self.state_dirty = true;
    }

    fn add_mic_channel(&mut self, source: &str) {
        let Some(slot) = self.free_slot() else { return };
        let id = {
            let existing: Vec<&str> = self
                .state
                .config
                .channels
                .iter()
                .map(|c| c.id.as_str())
                .collect();
            rules::unique_id("mic", &existing)
        };
        let mut mic = Channel::new(id.clone(), "Wave XLR");
        mic.kind = ChannelKind::Input;
        mic.source = Some(source.to_owned());
        mic.icon = "mic".into();
        mic.color = "#3b82f6".into();
        mic.monitor.muted = true;
        self.state.config.channels.insert(0, mic);
        self.slots[slot] = Some(id.clone());
        if let Some(cmd) = self.ensure_channel_cmd(&id) {
            self.backend.send(cmd);
        }
        self.push_targets();
        self.changed();
    }

    fn resync(&mut self) {
        for channel in &self.state.config.channels {
            if let Some(cmd) = self.ensure_channel_cmd(&channel.id) {
                self.backend.send(cmd);
            }
        }
        self.push_targets();
        self.backend.send(BackendCommand::SetMonitorOutput(
            self.state.config.mixes.monitor.output_device.clone(),
        ));
        self.backend
            .send(BackendCommand::SetDefaultSink(Some(self.fallback_node())));
        for app in &self.state.apps {
            if let Some(target) = rules::channel_for_app(&self.state.config.channels, &app.app)
                && app.channel.as_deref() != Some(target.id.as_str())
            {
                self.backend.send(BackendCommand::MoveStream {
                    node_id: app.node_id,
                    target: Some(target.node_name()),
                });
            }
        }
    }

    // ---- helpers shared with mixing.rs and channels.rs ----

    fn resolve_channel(&self, query: &str) -> Result<ChannelId, ErrorBody> {
        let candidates: Vec<Candidate> = self
            .state
            .config
            .channels
            .iter()
            .map(|c| Candidate {
                id: &c.id,
                name: &c.name,
            })
            .collect();
        resolve(query, &candidates)
            .map(str::to_owned)
            .map_err(ErrorBody::from)
    }

    /// Like `resolve_channel`, but rejects input (mic) channels: apps can only play
    /// into virtual channels.
    fn resolve_virtual_channel(&self, query: &str) -> Result<ChannelId, ErrorBody> {
        let id = self.resolve_channel(query)?;
        match self.state.config.channel(&id) {
            Some(c) if c.kind == ChannelKind::Input => Err(ErrorBody::invalid(format!(
                "'{}' is an input channel; apps can only play into virtual channels",
                c.name
            ))),
            _ => Ok(id),
        }
    }

    fn slot_of(&self, id: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.as_deref() == Some(id))
    }

    fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(Option::is_none)
    }

    fn push_targets(&self) {
        self.backend
            .send(BackendCommand::SetTargets(compute_targets(
                &self.state.config,
                &self.slots,
            )));
    }

    /// Marks both the persisted config and the broadcast state as changed.
    fn changed(&mut self) {
        self.config_dirty = true;
        self.state_dirty = true;
    }

    fn ensure_channel_cmd(&self, id: &str) -> Option<BackendCommand> {
        let slot = self.slot_of(id)?;
        let channel = self.state.config.channel(id)?;
        let source = match (channel.kind, &channel.source) {
            (ChannelKind::Input, Some(source)) => ChannelSource::Input {
                source: source.clone(),
            },
            _ => ChannelSource::Virtual,
        };
        Some(BackendCommand::EnsureChannel {
            id: id.to_owned(),
            slot,
            description: format!("MixLink {}", channel.name),
            source,
        })
    }

    fn fallback_node(&self) -> String {
        channel_node_name(&self.state.config.fallback_channel)
    }

    /// Moves every running stream of `app` into `channel`, or back to the default
    /// sink (the fallback channel) for `None`. Streams already playing into `channel`
    /// are left alone: re-targeting them would only cause a dropout.
    fn move_app_streams(&self, app: &str, channel: Option<&str>) {
        for active in self.state.apps.iter().filter(|a| a.app == app) {
            if channel.is_some() && active.channel.as_deref() == channel {
                continue;
            }
            self.backend.send(BackendCommand::MoveStream {
                node_id: active.node_id,
                target: channel.map(channel_node_name),
            });
        }
    }
}

fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("state is serializable")
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::backend::fake::FakeBackend;
    use crate::mixer::dsp::Peaks;
    use mixlink_core::model::{Channel, ChannelKind, DeviceKind};

    #[test]
    fn ready_resyncs_everything() {
        let (mut app, fake) = app();
        app.on_backend_event(BackendEvent::Ready {
            previous_default_sink: Some("alsa_output.headphones".into()),
        });
        let cmds = fake.take();
        assert_eq!(
            cmds[0],
            BackendCommand::EnsureChannel {
                id: "system".into(),
                slot: 0,
                description: "MixLink System".into(),
                source: ChannelSource::Virtual
            }
        );
        assert!(
            matches!(&cmds[2], BackendCommand::EnsureChannel { id, slot: 2, .. } if id == "voice-chat")
        );
        assert!(matches!(cmds[3], BackendCommand::SetTargets(_)));
        assert_eq!(
            cmds[4],
            BackendCommand::SetMonitorOutput(Some("alsa_output.headphones".into()))
        );
        assert_eq!(
            cmds[5],
            BackendCommand::SetDefaultSink(Some("mixlink.channel.system".into()))
        );
        assert_eq!(
            app.state().config.mixes.monitor.output_device.as_deref(),
            Some("alsa_output.headphones")
        );
        assert!(app.take_config_dirty());
    }

    #[test]
    fn ready_ignores_our_own_node_as_previous_default() {
        let (mut app, fake) = app();
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            "alsa_output.speakers",
            "Speakers",
            DeviceKind::Sink,
        )));
        app.on_backend_event(BackendEvent::Ready {
            previous_default_sink: Some("mixlink.channel.system".into()),
        });
        assert_eq!(
            app.state().config.mixes.monitor.output_device.as_deref(),
            Some("alsa_output.speakers"),
            "falls back to the first sink device"
        );
        app.shutdown();
        let cmds = fake.take();
        assert_eq!(cmds[cmds.len() - 2], BackendCommand::SetDefaultSink(None));
        assert_eq!(cmds[cmds.len() - 1], BackendCommand::Shutdown);
    }

    #[test]
    fn new_stream_with_rule_is_moved_once() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.system"),
        )));
        assert_eq!(
            moves(&fake.take()),
            vec![(40, Some("mixlink.channel.music".into()))]
        );
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.system"),
        )));
        assert!(
            moves(&fake.take()).is_empty(),
            "known streams are not moved again"
        );
        assert_eq!(app.state().apps[0].channel.as_deref(), Some("system"));
    }

    #[test]
    fn stream_without_rule_or_already_in_place_is_not_moved() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            41,
            "firefox",
            Some("alsa_output.x"),
        )));
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            42,
            "spotify",
            Some("mixlink.channel.music"),
        )));
        assert!(moves(&fake.take()).is_empty());
        assert_eq!(app.state().apps.len(), 2);
        assert!(app.take_state_dirty());
    }

    #[test]
    fn stream_removed_drops_app() {
        let (mut app, _fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(41, "firefox", None)));
        app.on_backend_event(BackendEvent::StreamRemoved(41));
        assert!(app.state().apps.is_empty());
    }

    #[test]
    fn first_run_adds_mic_channel_for_wave_xlr() {
        let fake = FakeBackend::new();
        let mut app = App::new(config(), fake.clone(), true);
        let name = "alsa_input.usb-Elgato_Systems_Elgato_Wave_XLR_0000000000-00.mono-fallback";
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            name,
            "Wave XLR",
            DeviceKind::Source,
        )));
        let mic = &app.state().config.channels[0];
        assert_eq!(mic.id, "mic");
        assert_eq!(mic.kind, ChannelKind::Input);
        assert_eq!(mic.source.as_deref(), Some(name));
        assert!(mic.monitor.muted);
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::EnsureChannel {
            id: "mic".into(),
            slot: 3,
            description: "MixLink Wave XLR".into(),
            source: ChannelSource::Input {
                source: name.into()
            },
        }));
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            name,
            "Wave XLR",
            DeviceKind::Source,
        )));
        assert_eq!(app.state().config.channels.len(), 4, "only one mic channel");
    }

    const XLR_SINK: &str =
        "alsa_output.usb-Elgato_Systems_Elgato_Wave_XLR_0000000000-00.analog-stereo";
    const XLR_SOURCE: &str =
        "alsa_input.usb-Elgato_Systems_Elgato_Wave_XLR_0000000000-00.mono-fallback";

    #[test]
    fn first_run_prefers_wave_xlr_headphones_for_monitor() {
        let fake = FakeBackend::new();
        let mut app = App::new(config(), fake.clone(), true);
        // Devices are reported before Ready.
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            XLR_SINK,
            "Wave XLR",
            DeviceKind::Sink,
        )));
        assert!(
            fake.take()
                .contains(&BackendCommand::SetMonitorOutput(Some(XLR_SINK.into())))
        );
        app.on_backend_event(BackendEvent::Ready {
            previous_default_sink: Some("alsa_output.speakers".into()),
        });
        assert_eq!(
            app.state().config.mixes.monitor.output_device.as_deref(),
            Some(XLR_SINK)
        );
        assert!(
            fake.take()
                .contains(&BackendCommand::SetMonitorOutput(Some(XLR_SINK.into())))
        );
    }

    #[test]
    fn first_run_magic_ends_with_the_first_ready() {
        let fake = FakeBackend::new();
        let mut c = config();
        c.channels.retain(|ch| ch.kind != ChannelKind::Input);
        let mut app = App::new(c, fake.clone(), true);
        let xlr = device(XLR_SINK, "Wave XLR", DeviceKind::Sink);
        app.on_backend_event(BackendEvent::DeviceAdded(xlr.clone()));
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            "alsa_output.speakers",
            "Speakers",
            DeviceKind::Sink,
        )));
        app.on_backend_event(BackendEvent::Ready {
            previous_default_sink: Some("alsa_output.speakers".into()),
        });
        app.handle(Command::SetMix {
            mix: mixlink_core::model::MixKind::Monitor,
            volume: None,
            delta: None,
            muted: None,
            output_device: Some("speakers".into()),
        })
        .unwrap();
        fake.take();

        // The XLR is replugged: the user's choice stays.
        app.on_backend_event(BackendEvent::DeviceRemoved(XLR_SINK.into()));
        app.on_backend_event(BackendEvent::DeviceAdded(xlr));
        assert_eq!(
            app.state().config.mixes.monitor.output_device.as_deref(),
            Some("alsa_output.speakers")
        );
        // A Wave XLR mic showing up later does not create a mic channel either.
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            XLR_SOURCE,
            "Wave XLR",
            DeviceKind::Source,
        )));
        assert_eq!(app.state().config.channels.len(), 3);
        assert!(!fake.take().iter().any(|c| matches!(
            c,
            BackendCommand::SetMonitorOutput(_) | BackendCommand::EnsureChannel { .. }
        )));
    }

    #[test]
    fn devices_are_tracked_without_first_run_magic() {
        let (mut app, _fake) = ready_app();
        let name = "alsa_input.usb-Elgato_Systems_Elgato_Wave_XLR_0000000000-00.mono-fallback";
        app.on_backend_event(BackendEvent::DeviceAdded(device(
            name,
            "Wave XLR",
            DeviceKind::Source,
        )));
        assert_eq!(app.state().devices.len(), 1);
        assert_eq!(app.state().config.channels.len(), 3);
        app.on_backend_event(BackendEvent::DeviceRemoved(name.into()));
        assert!(app.state().devices.is_empty());
    }

    #[test]
    fn meters_are_mapped_to_channel_ids() {
        let (mut app, _fake) = ready_app();
        let mut peaks = Peaks::default();
        peaks.channels[1] = [0.5, 0.25];
        peaks.stream = [0.1, 0.2];
        match app.on_backend_event(BackendEvent::Meters(peaks)) {
            Some(Event::Meters {
                channels, stream, ..
            }) => {
                assert_eq!(channels["music"], [0.5, 0.25]);
                assert_eq!(channels.len(), 3);
                assert_eq!(stream, [0.1, 0.2]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn disconnect_clears_runtime_state() {
        let (mut app, _fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(41, "firefox", None)));
        app.on_backend_event(BackendEvent::Disconnected);
        assert!(app.state().apps.is_empty());
    }

    #[test]
    fn too_many_channels_are_truncated_with_warning() {
        let mut c = config();
        c.channels = (0..20)
            .map(|i| Channel::new(format!("c{i}"), format!("C{i}")))
            .collect();
        c.fallback_channel = "c0".into();
        let app = App::new(c, FakeBackend::new(), false);
        assert_eq!(app.state().config.channels.len(), MAX_CHANNELS);
        assert_eq!(app.state().warnings.len(), 1);
    }

    #[test]
    fn read_only_commands_return_state_parts() {
        let (mut app, _fake) = ready_app();
        let state = app.handle(Command::GetState).unwrap();
        assert_eq!(state["config"]["fallback_channel"], "system");
        assert_eq!(
            app.handle(Command::ListApps).unwrap(),
            serde_json::json!([])
        );
    }
}
