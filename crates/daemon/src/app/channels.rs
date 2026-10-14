//! Channel management and app assignment commands.
use serde_json::json;
use wavelink_core::model::{channel_node_name, Channel, ChannelId, MAX_CHANNELS};
use wavelink_core::protocol::{ErrorBody, ErrorCode};
use wavelink_core::resolve::{resolve, Candidate};
use wavelink_core::rules;

use super::{to_value, App, CmdResult};
use crate::backend::{AudioBackend, BackendCommand};

impl<B: AudioBackend> App<B> {
    pub(super) fn assign_app(&mut self, app: &str, channel: &str) -> CmdResult {
        if app.trim().is_empty() {
            return Err(ErrorBody::invalid("app name must not be empty"));
        }
        let app = self.app_identity_for(app);
        let id = self.resolve_channel(channel)?;
        rules::assign_app(&mut self.state.config.channels, &app, &id);
        self.move_app_streams(&app, &channel_node_name(&id));
        self.changed();
        Ok(to_value(self.state.config.channel(&id).expect("resolved channel exists")))
    }

    pub(super) fn unassign_app(&mut self, app: &str) -> CmdResult {
        let assigned = rules::assigned_apps(&self.state.config.channels);
        let candidates: Vec<Candidate> = assigned.iter().map(|a| Candidate { id: a, name: a }).collect();
        let app = resolve(app, &candidates).map_err(ErrorBody::from)?.to_owned();
        rules::unassign_app(&mut self.state.config.channels, &app);
        self.move_app_streams(&app, &self.fallback_node());
        self.changed();
        Ok(json!({ "app": app, "channel": self.state.config.fallback_channel }))
    }

    pub(super) fn create_channel(&mut self, name: &str, icon: Option<String>, color: Option<String>) -> CmdResult {
        let name = name.trim();
        if name.is_empty() {
            return Err(ErrorBody::invalid("channel name must not be empty"));
        }
        let Some(slot) = self.free_slot() else {
            return Err(ErrorBody::new(ErrorCode::LimitReached, format!("at most {MAX_CHANNELS} channels are supported")));
        };
        let id = {
            let existing: Vec<&str> = self.state.config.channels.iter().map(|c| c.id.as_str()).collect();
            rules::unique_id(name, &existing)
        };
        let mut channel = Channel::new(id.clone(), name);
        if let Some(icon) = icon {
            channel.icon = icon;
        }
        if let Some(color) = color {
            channel.color = color;
        }
        self.state.config.channels.push(channel);
        self.slots[slot] = Some(id.clone());
        if let Some(cmd) = self.ensure_channel_cmd(&id) {
            self.backend.send(cmd);
        }
        self.push_targets();
        self.changed();
        Ok(to_value(self.state.config.channel(&id).expect("just created")))
    }

    pub(super) fn rename_channel(&mut self, channel: &str, name: &str) -> CmdResult {
        let name = name.trim();
        if name.is_empty() {
            return Err(ErrorBody::invalid("channel name must not be empty"));
        }
        let id = self.resolve_channel(channel)?;
        self.state.config.channel_mut(&id).expect("resolved channel exists").name = name.to_owned();
        // The sink is recreated with the new description, so its streams need to be routed back.
        if let Some(cmd) = self.ensure_channel_cmd(&id) {
            self.backend.send(cmd);
        }
        let node = channel_node_name(&id);
        for app in self.state.apps.iter().filter(|a| a.channel.as_deref() == Some(id.as_str())) {
            self.backend.send(BackendCommand::MoveStream { node_id: app.node_id, target: node.clone() });
        }
        self.changed();
        Ok(to_value(self.state.config.channel(&id).expect("resolved channel exists")))
    }

    pub(super) fn update_channel(&mut self, channel: &str, icon: Option<String>, color: Option<String>) -> CmdResult {
        if icon.is_none() && color.is_none() {
            return Err(ErrorBody::invalid("nothing to change: give icon or color"));
        }
        let id = self.resolve_channel(channel)?;
        let c = self.state.config.channel_mut(&id).expect("resolved channel exists");
        if let Some(icon) = icon {
            c.icon = icon;
        }
        if let Some(color) = color {
            c.color = color;
        }
        let result = to_value(&*c);
        self.changed();
        Ok(result)
    }

    pub(super) fn delete_channel(&mut self, channel: &str) -> CmdResult {
        let id = self.resolve_channel(channel)?;
        if id == self.state.config.fallback_channel {
            return Err(ErrorBody::new(
                ErrorCode::NotAllowed,
                "cannot delete the fallback channel; choose another fallback channel first",
            ));
        }
        let index = self.state.config.channels.iter().position(|c| c.id == id).expect("resolved channel exists");
        let removed = self.state.config.channels.remove(index);
        if let Some(slot) = self.slot_of(&id) {
            self.slots[slot] = None;
        }
        self.backend.send(BackendCommand::RemoveChannel { id: id.clone() });
        let fallback = self.fallback_node();
        for app in &self.state.apps {
            if app.channel.as_deref() == Some(id.as_str()) || removed.apps.contains(&app.app) {
                self.backend.send(BackendCommand::MoveStream { node_id: app.node_id, target: fallback.clone() });
            }
        }
        self.push_targets();
        self.changed();
        Ok(json!({ "deleted": id }))
    }

    pub(super) fn reorder_channels(&mut self, order: &[String]) -> CmdResult {
        let mut resolved: Vec<ChannelId> = Vec::with_capacity(order.len());
        for query in order {
            resolved.push(self.resolve_channel(query)?);
        }
        let mut given = resolved.clone();
        given.sort();
        given.dedup();
        let mut all: Vec<ChannelId> = self.state.config.channels.iter().map(|c| c.id.clone()).collect();
        all.sort();
        if given != all || resolved.len() != all.len() {
            return Err(ErrorBody::invalid("order must list every channel exactly once"));
        }
        self.state
            .config
            .channels
            .sort_by_key(|c| resolved.iter().position(|id| *id == c.id).expect("validated above"));
        self.changed();
        Ok(json!(resolved))
    }

    /// Maps a user-supplied app name to a running app's identity when possible,
    /// otherwise to the normalized name (so apps can be assigned before they run).
    fn app_identity_for(&self, query: &str) -> String {
        let mut seen = std::collections::HashSet::new();
        let candidates: Vec<Candidate> = self
            .state
            .apps
            .iter()
            .filter(|a| seen.insert(a.app.as_str()))
            .map(|a| Candidate { id: &a.app, name: &a.name })
            .collect();
        resolve(query, &candidates).map(str::to_owned).unwrap_or_else(|_| rules::normalize_app(query))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use wavelink_core::model::MAX_CHANNELS;
    use wavelink_core::protocol::{Command, ErrorCode};

    use crate::backend::{BackendCommand, BackendEvent, ChannelSource};

    fn create(name: &str) -> Command {
        Command::CreateChannel { name: name.into(), icon: None, color: None }
    }

    #[test]
    fn create_channel_uses_slug_and_free_slot() {
        let (mut app, fake) = ready_app();
        let result = app.handle(create("Podcast")).unwrap();
        assert_eq!(result["id"], "podcast");
        assert!(fake.take().contains(&BackendCommand::EnsureChannel {
            id: "podcast".into(),
            slot: 3,
            description: "WaveLink Podcast".into(),
            source: ChannelSource::Virtual,
        }));
        assert_eq!(app.handle(create("Podcast")).unwrap()["id"], "podcast-2");
    }

    #[test]
    fn create_channel_validates_name_and_limit() {
        let (mut app, _fake) = ready_app();
        assert_eq!(app.handle(create("  ")).unwrap_err().code, ErrorCode::InvalidArgument);
        for i in 3..MAX_CHANNELS {
            app.handle(create(&format!("Extra {i}"))).unwrap();
        }
        assert_eq!(app.handle(create("One too many")).unwrap_err().code, ErrorCode::LimitReached);
    }

    #[test]
    fn rename_keeps_id_and_reroutes_streams() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(40, "spotify", Some("wavelink.channel.music"))));
        fake.take();
        app.handle(Command::RenameChannel { channel: "music".into(), name: "Tunes".into() }).unwrap();
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::EnsureChannel {
            id: "music".into(),
            slot: 1,
            description: "WaveLink Tunes".into(),
            source: ChannelSource::Virtual,
        }));
        assert_eq!(moves(&cmds), vec![(40, "wavelink.channel.music".into())]);
        assert_eq!(app.state().config.channel("music").unwrap().name, "Tunes");
    }

    #[test]
    fn update_channel_needs_a_field() {
        let (mut app, _fake) = ready_app();
        let err = app.handle(Command::UpdateChannel { channel: "music".into(), icon: None, color: None }).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        app.handle(Command::UpdateChannel { channel: "music".into(), icon: Some("music".into()), color: None }).unwrap();
        assert_eq!(app.state().config.channel("music").unwrap().icon, "music");
    }

    #[test]
    fn fallback_channel_cannot_be_deleted() {
        let (mut app, _fake) = ready_app();
        let err = app.handle(Command::DeleteChannel { channel: "system".into() }).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotAllowed);
    }

    #[test]
    fn delete_frees_slot_and_moves_streams_to_fallback() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(40, "spotify", Some("wavelink.channel.music"))));
        fake.take();
        app.handle(Command::DeleteChannel { channel: "music".into() }).unwrap();
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::RemoveChannel { id: "music".into() }));
        assert_eq!(moves(&cmds), vec![(40, "wavelink.channel.system".into())]);
        assert!(app.state().config.channel("music").is_none());
        app.handle(create("New")).unwrap();
        assert!(fake.take().iter().any(|c| matches!(c, BackendCommand::EnsureChannel { slot: 1, .. })));
    }

    #[test]
    fn reorder_requires_every_channel_once() {
        let (mut app, _fake) = ready_app();
        let order = |ids: &[&str]| Command::ReorderChannels { order: ids.iter().map(|s| s.to_string()).collect() };
        assert_eq!(app.handle(order(&["music", "system"])).unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(app.handle(order(&["music", "music", "system"])).unwrap_err().code, ErrorCode::InvalidArgument);
        app.handle(order(&["voice", "music", "system"])).unwrap();
        let ids: Vec<&str> = app.state().config.channels.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["voice-chat", "music", "system"]);
    }

    #[test]
    fn assign_moves_running_streams_and_uses_active_app_names() {
        let (mut app, fake) = ready_app();
        let mut info = stream(50, "discord", Some("wavelink.channel.system"));
        info.name = "Discord".into();
        app.on_backend_event(BackendEvent::StreamUpdated(info));
        fake.take();
        app.handle(Command::AssignApp { app: "Discord".into(), channel: "voice".into() }).unwrap();
        assert_eq!(moves(&fake.take()), vec![(50, "wavelink.channel.voice-chat".into())]);
        assert_eq!(app.state().config.channel("voice-chat").unwrap().apps, vec!["discord"]);
    }

    #[test]
    fn assign_accepts_apps_that_are_not_running() {
        let (mut app, _fake) = ready_app();
        app.handle(Command::AssignApp { app: "OBS".into(), channel: "music".into() }).unwrap();
        assert_eq!(app.state().config.channel("music").unwrap().apps, vec!["spotify", "obs"]);
    }

    #[test]
    fn unassign_moves_streams_to_fallback_and_reports_unknown_apps() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(40, "spotify", Some("wavelink.channel.music"))));
        fake.take();
        let err = app.handle(Command::UnassignApp { app: "zzz".into() }).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.options, vec!["spotify"]);
        app.handle(Command::UnassignApp { app: "spot".into() }).unwrap();
        assert_eq!(moves(&fake.take()), vec![(40, "wavelink.channel.system".into())]);
        assert!(app.state().config.channel("music").unwrap().apps.is_empty());
    }
}
