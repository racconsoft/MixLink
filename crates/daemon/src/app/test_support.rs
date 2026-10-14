use mixlink_core::model::{Channel, Config, Device, DeviceKind, MixKind};

use super::App;
use crate::backend::fake::FakeBackend;
use crate::backend::{BackendCommand, BackendEvent, StreamInfo};
use crate::mixer::dsp::Targets;

/// system (fallback, slot 0), music (slot 1, rule "spotify"), voice-chat (slot 2).
pub fn config() -> Config {
    let mut music = Channel::new("music", "Music");
    music.apps = vec!["spotify".into()];
    Config {
        version: 1,
        fallback_channel: "system".into(),
        listen: MixKind::Monitor,
        settings: Default::default(),
        mixes: Default::default(),
        channels: vec![
            Channel::new("system", "System"),
            music,
            Channel::new("voice-chat", "Voice Chat"),
        ],
    }
}

pub fn app() -> (App<FakeBackend>, FakeBackend) {
    let fake = FakeBackend::new();
    (App::new(config(), fake.clone(), false), fake)
}

/// An app that has seen `Ready` and has empty dirty flags and command log.
pub fn ready_app() -> (App<FakeBackend>, FakeBackend) {
    let (mut app, fake) = app();
    app.on_backend_event(BackendEvent::Ready {
        previous_default_sink: Some("alsa_output.headphones".into()),
    });
    fake.take();
    app.take_config_dirty();
    app.take_state_dirty();
    (app, fake)
}

pub fn stream(node_id: u32, app: &str, target: Option<&str>) -> StreamInfo {
    StreamInfo {
        node_id,
        app: app.into(),
        name: app.into(),
        icon: None,
        target: target.map(Into::into),
    }
}

pub fn device(name: &str, description: &str, kind: DeviceKind) -> Device {
    Device {
        name: name.into(),
        description: description.into(),
        kind,
    }
}

pub fn last_targets(cmds: &[BackendCommand]) -> Targets {
    cmds.iter()
        .rev()
        .find_map(|c| {
            if let BackendCommand::SetTargets(t) = c {
                Some(*t)
            } else {
                None
            }
        })
        .expect("no SetTargets sent")
}

/// `(node_id, target)` of every MoveStream; `None` means "follow the default sink".
pub fn moves(cmds: &[BackendCommand]) -> Vec<(u32, Option<String>)> {
    cmds.iter()
        .filter_map(|c| match c {
            BackendCommand::MoveStream { node_id, target } => Some((*node_id, target.clone())),
            _ => None,
        })
        .collect()
}
