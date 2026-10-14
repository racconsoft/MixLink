//! Channel management and app assignment commands.
use mixlink_core::model::{Channel, ChannelId, MAX_CHANNELS, channel_node_name};
use mixlink_core::protocol::{ErrorBody, ErrorCode};
use mixlink_core::resolve::{Candidate, ResolveError, resolve};
use mixlink_core::rules;
use serde_json::json;

use super::{App, CmdResult, to_value};
use crate::backend::{AudioBackend, BackendCommand};

impl<B: AudioBackend> App<B> {
    pub(super) fn assign_app(&mut self, app: &str, channel: &str) -> CmdResult {
        if app.trim().is_empty() {
            return Err(ErrorBody::invalid("app name must not be empty"));
        }
        let app = self.app_identity_for(app)?;
        let id = self.resolve_virtual_channel(channel)?;
        rules::assign_app(&mut self.state.config.channels, &app, &id);
        self.move_app_streams(&app, Some(&id));
        self.changed();
        Ok(to_value(
            self.state
                .config
                .channel(&id)
                .expect("resolved channel exists"),
        ))
    }

    pub(super) fn unassign_app(&mut self, app: &str) -> CmdResult {
        let assigned = rules::assigned_apps(&self.state.config.channels);
        let candidates: Vec<Candidate> = assigned
            .iter()
            .map(|a| Candidate { id: a, name: a })
            .collect();
        let app = resolve(app, &candidates)
            .map_err(ErrorBody::from)?
            .to_owned();
        rules::unassign_app(&mut self.state.config.channels, &app);
        self.move_app_streams(&app, None);
        self.changed();
        Ok(json!({ "app": app, "channel": self.state.config.fallback_channel }))
    }

    pub(super) fn create_channel(
        &mut self,
        name: &str,
        icon: Option<String>,
        color: Option<String>,
    ) -> CmdResult {
        let name = validate_channel_name(name)?;
        let Some(slot) = self.free_slot() else {
            return Err(ErrorBody::new(
                ErrorCode::LimitReached,
                format!("at most {MAX_CHANNELS} channels are supported"),
            ));
        };
        let id = {
            let existing: Vec<&str> = self
                .state
                .config
                .channels
                .iter()
                .map(|c| c.id.as_str())
                .collect();
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
        Ok(to_value(
            self.state.config.channel(&id).expect("just created"),
        ))
    }

    pub(super) fn rename_channel(&mut self, channel: &str, name: &str) -> CmdResult {
        let name = validate_channel_name(name)?;
        let id = self.resolve_channel(channel)?;
        self.state
            .config
            .channel_mut(&id)
            .expect("resolved channel exists")
            .name = name.to_owned();
        // The sink is recreated with the new description, so its streams need to be routed back.
        if let Some(cmd) = self.ensure_channel_cmd(&id) {
            self.backend.send(cmd);
        }
        // Streams on the fallback channel go back to following the default sink, so
        // they are not pinned to it.
        let target = (id != self.state.config.fallback_channel).then(|| channel_node_name(&id));
        for app in self
            .state
            .apps
            .iter()
            .filter(|a| a.channel.as_deref() == Some(id.as_str()))
        {
            self.backend.send(BackendCommand::MoveStream {
                node_id: app.node_id,
                target: target.clone(),
            });
        }
        self.changed();
        Ok(to_value(
            self.state
                .config
                .channel(&id)
                .expect("resolved channel exists"),
        ))
    }

    pub(super) fn update_channel(
        &mut self,
        channel: &str,
        icon: Option<String>,
        color: Option<String>,
    ) -> CmdResult {
        if icon.is_none() && color.is_none() {
            return Err(ErrorBody::invalid("nothing to change: give icon or color"));
        }
        let id = self.resolve_channel(channel)?;
        let c = self
            .state
            .config
            .channel_mut(&id)
            .expect("resolved channel exists");
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
        let index = self
            .state
            .config
            .channels
            .iter()
            .position(|c| c.id == id)
            .expect("resolved channel exists");
        let removed = self.state.config.channels.remove(index);
        if let Some(slot) = self.slot_of(&id) {
            self.slots[slot] = None;
        }
        self.backend
            .send(BackendCommand::RemoveChannel { id: id.clone() });
        for app in &self.state.apps {
            if app.channel.as_deref() == Some(id.as_str()) || removed.apps.contains(&app.app) {
                // Back to the default sink, i.e. the fallback channel, without pinning.
                self.backend.send(BackendCommand::MoveStream {
                    node_id: app.node_id,
                    target: None,
                });
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
        let mut all: Vec<ChannelId> = self
            .state
            .config
            .channels
            .iter()
            .map(|c| c.id.clone())
            .collect();
        all.sort();
        if given != all || resolved.len() != all.len() {
            return Err(ErrorBody::invalid(
                "order must list every channel exactly once",
            ));
        }
        self.state.config.channels.sort_by_key(|c| {
            resolved
                .iter()
                .position(|id| *id == c.id)
                .expect("validated above")
        });
        self.changed();
        Ok(json!(resolved))
    }

    /// Maps a user-supplied app name to a running app's identity when it matches that
    /// identity or the app's display name exactly (ignoring case); otherwise to the
    /// normalized name, so apps can be assigned before they run. No fuzzy matching:
    /// an assignment rule is persistent, and a guessed app would be a surprise.
    /// Two different running apps sharing the queried display name are ambiguous.
    fn app_identity_for(&self, query: &str) -> Result<String, ErrorBody> {
        let normalized = rules::normalize_app(query);
        if self
            .state
            .apps
            .iter()
            .any(|a| a.app.to_lowercase() == normalized)
        {
            return Ok(normalized);
        }
        let mut matches: Vec<&str> = self
            .state
            .apps
            .iter()
            .filter(|a| a.name.trim().to_lowercase() == normalized)
            .map(|a| a.app.as_str())
            .collect();
        matches.sort_unstable();
        matches.dedup();
        match matches.as_slice() {
            [] => Ok(normalized),
            [app] => Ok((*app).to_owned()),
            _ => Err(ErrorBody::from(ResolveError::Ambiguous {
                query: query.to_owned(),
                options: matches.iter().map(|a| (*a).to_owned()).collect(),
            })),
        }
    }
}

/// Trims a channel name and rejects empty names and control characters (PipeWire
/// properties cannot carry e.g. NUL).
fn validate_channel_name(name: &str) -> Result<&str, ErrorBody> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorBody::invalid("channel name must not be empty"));
    }
    if name.chars().any(char::is_control) {
        return Err(ErrorBody::invalid(
            "channel name must not contain control characters",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::super::App;
    use super::super::test_support::*;
    use mixlink_core::model::{Channel, ChannelKind, MAX_CHANNELS};
    use mixlink_core::protocol::{Command, ErrorCode};

    use crate::backend::fake::FakeBackend;
    use crate::backend::{BackendCommand, BackendEvent, ChannelSource};

    fn create(name: &str) -> Command {
        Command::CreateChannel {
            name: name.into(),
            icon: None,
            color: None,
        }
    }

    #[test]
    fn create_channel_uses_slug_and_free_slot() {
        let (mut app, fake) = ready_app();
        let result = app.handle(create("Podcast")).unwrap();
        assert_eq!(result["id"], "podcast");
        assert!(fake.take().contains(&BackendCommand::EnsureChannel {
            id: "podcast".into(),
            slot: 3,
            description: "MixLink Podcast".into(),
            source: ChannelSource::Virtual,
        }));
        assert_eq!(app.handle(create("Podcast")).unwrap()["id"], "podcast-2");
    }

    #[test]
    fn create_channel_validates_name_and_limit() {
        let (mut app, _fake) = ready_app();
        assert_eq!(
            app.handle(create("  ")).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        for i in 3..MAX_CHANNELS {
            app.handle(create(&format!("Extra {i}"))).unwrap();
        }
        assert_eq!(
            app.handle(create("One too many")).unwrap_err().code,
            ErrorCode::LimitReached
        );
    }

    #[test]
    fn rename_keeps_id_and_reroutes_streams() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.music"),
        )));
        fake.take();
        app.handle(Command::RenameChannel {
            channel: "music".into(),
            name: "Tunes".into(),
        })
        .unwrap();
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::EnsureChannel {
            id: "music".into(),
            slot: 1,
            description: "MixLink Tunes".into(),
            source: ChannelSource::Virtual,
        }));
        assert_eq!(
            moves(&cmds),
            vec![(40, Some("mixlink.channel.music".into()))]
        );
        assert_eq!(app.state().config.channel("music").unwrap().name, "Tunes");
    }

    #[test]
    fn renaming_the_fallback_channel_unpins_its_streams() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            41,
            "firefox",
            Some("mixlink.channel.system"),
        )));
        fake.take();
        app.handle(Command::RenameChannel {
            channel: "system".into(),
            name: "Everything".into(),
        })
        .unwrap();
        assert_eq!(moves(&fake.take()), vec![(41, None)]);
    }

    #[test]
    fn update_channel_needs_a_field() {
        let (mut app, _fake) = ready_app();
        let err = app
            .handle(Command::UpdateChannel {
                channel: "music".into(),
                icon: None,
                color: None,
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        app.handle(Command::UpdateChannel {
            channel: "music".into(),
            icon: Some("music".into()),
            color: None,
        })
        .unwrap();
        assert_eq!(app.state().config.channel("music").unwrap().icon, "music");
    }

    #[test]
    fn fallback_channel_cannot_be_deleted() {
        let (mut app, _fake) = ready_app();
        let err = app
            .handle(Command::DeleteChannel {
                channel: "system".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotAllowed);
    }

    #[test]
    fn delete_frees_slot_and_moves_streams_to_fallback() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.music"),
        )));
        fake.take();
        app.handle(Command::DeleteChannel {
            channel: "music".into(),
        })
        .unwrap();
        let cmds = fake.take();
        assert!(cmds.contains(&BackendCommand::RemoveChannel { id: "music".into() }));
        assert_eq!(moves(&cmds), vec![(40, None)], "back to the default sink");
        assert!(app.state().config.channel("music").is_none());
        app.handle(create("New")).unwrap();
        assert!(
            fake.take()
                .iter()
                .any(|c| matches!(c, BackendCommand::EnsureChannel { slot: 1, .. }))
        );
    }

    #[test]
    fn reorder_requires_every_channel_once() {
        let (mut app, _fake) = ready_app();
        let order = |ids: &[&str]| Command::ReorderChannels {
            order: ids.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(
            app.handle(order(&["music", "system"])).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            app.handle(order(&["music", "music", "system"]))
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
        app.handle(order(&["voice", "music", "system"])).unwrap();
        let ids: Vec<&str> = app
            .state()
            .config
            .channels
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(ids, vec!["voice-chat", "music", "system"]);
    }

    #[test]
    fn assign_moves_running_streams_and_uses_active_app_names() {
        let (mut app, fake) = ready_app();
        let mut info = stream(50, "discord", Some("mixlink.channel.system"));
        info.name = "Discord".into();
        app.on_backend_event(BackendEvent::StreamUpdated(info));
        fake.take();
        app.handle(Command::AssignApp {
            app: "Discord".into(),
            channel: "voice".into(),
        })
        .unwrap();
        assert_eq!(
            moves(&fake.take()),
            vec![(50, Some("mixlink.channel.voice-chat".into()))]
        );
        assert_eq!(
            app.state().config.channel("voice-chat").unwrap().apps,
            vec!["discord"]
        );
    }

    #[test]
    fn assign_accepts_apps_that_are_not_running() {
        let (mut app, _fake) = ready_app();
        app.handle(Command::AssignApp {
            app: "OBS".into(),
            channel: "music".into(),
        })
        .unwrap();
        assert_eq!(
            app.state().config.channel("music").unwrap().apps,
            vec!["spotify", "obs"]
        );
    }

    #[test]
    fn assign_does_not_guess_running_apps() {
        let (mut app, fake) = ready_app();
        let mut discord = stream(50, "discord", Some("mixlink.channel.system"));
        discord.name = "Discord".into();
        let mut discordance = stream(51, "discordance", Some("mixlink.channel.system"));
        discordance.name = "Discordance".into();
        app.on_backend_event(BackendEvent::StreamUpdated(discord));
        app.on_backend_event(BackendEvent::StreamUpdated(discordance));
        fake.take();
        app.handle(Command::AssignApp {
            app: "disc".into(),
            channel: "voice".into(),
        })
        .unwrap();
        assert_eq!(
            app.state().config.channel("voice-chat").unwrap().apps,
            vec!["disc"],
            "a prefix is taken literally"
        );
        assert!(moves(&fake.take()).is_empty());
        app.handle(Command::AssignApp {
            app: "DISCORDANCE".into(),
            channel: "voice".into(),
        })
        .unwrap();
        assert_eq!(
            moves(&fake.take()),
            vec![(51, Some("mixlink.channel.voice-chat".into()))]
        );
    }

    #[test]
    fn assign_rejects_display_names_shared_by_different_apps() {
        let (mut app, fake) = ready_app();
        let mut a = stream(50, "game-a", Some("mixlink.channel.system"));
        a.name = "Game".into();
        let mut b = stream(51, "game-b", Some("mixlink.channel.system"));
        b.name = "Game".into();
        app.on_backend_event(BackendEvent::StreamUpdated(a));
        app.on_backend_event(BackendEvent::StreamUpdated(b));
        fake.take();
        let err = app
            .handle(Command::AssignApp {
                app: "game".into(),
                channel: "music".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Ambiguous);
        assert_eq!(err.options, vec!["game-a", "game-b"]);
        assert!(
            fake.take().is_empty(),
            "nothing moved on an ambiguous assign"
        );
        assert_eq!(
            app.state().config.channel("music").unwrap().apps,
            vec!["spotify"]
        );
    }

    #[test]
    fn assign_to_the_current_channel_does_not_move_the_stream() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.music"),
        )));
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            41,
            "spotify",
            Some("mixlink.channel.system"),
        )));
        fake.take();
        app.handle(Command::AssignApp {
            app: "spotify".into(),
            channel: "music".into(),
        })
        .unwrap();
        assert_eq!(
            moves(&fake.take()),
            vec![(41, Some("mixlink.channel.music".into()))],
            "only the stream not yet in Music is moved"
        );
    }

    fn mic_app() -> (App<FakeBackend>, FakeBackend) {
        let (mut app, fake) = ready_app();
        let mut mic = Channel::new("mic", "Wave XLR");
        mic.kind = ChannelKind::Input;
        mic.source = Some("alsa_input.xlr".into());
        app.state.config.channels.push(mic);
        (app, fake)
    }

    #[test]
    fn input_channels_cannot_take_apps() {
        let (mut app, fake) = mic_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "firefox",
            Some("mixlink.channel.system"),
        )));
        fake.take();
        let err = app
            .handle(Command::AssignApp {
                app: "firefox".into(),
                channel: "mic".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert!(err.message.contains("input channel"), "{}", err.message);
        assert!(app.state().config.channel("mic").unwrap().apps.is_empty());
        assert!(fake.take().is_empty());
    }

    #[test]
    fn input_channel_cannot_be_the_fallback() {
        let (mut app, fake) = mic_app();
        let err = app
            .handle(Command::SetFallbackChannel {
                channel: "wave xlr".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert!(err.message.contains("input channel"), "{}", err.message);
        assert_eq!(app.state().config.fallback_channel, "system");
        assert!(fake.take().is_empty());
    }

    #[test]
    fn channel_names_with_control_characters_are_rejected() {
        let (mut app, fake) = ready_app();
        for name in ["Bad\0Name", "Tab\tName", "Line\nBreak"] {
            assert_eq!(
                app.handle(create(name)).unwrap_err().code,
                ErrorCode::InvalidArgument
            );
            assert_eq!(
                app.handle(Command::RenameChannel {
                    channel: "music".into(),
                    name: name.into(),
                })
                .unwrap_err()
                .code,
                ErrorCode::InvalidArgument
            );
        }
        assert_eq!(app.state().config.channels.len(), 3);
        assert_eq!(app.state().config.channel("music").unwrap().name, "Music");
        assert!(fake.take().is_empty());
    }

    #[test]
    fn unassign_moves_streams_to_fallback_and_reports_unknown_apps() {
        let (mut app, fake) = ready_app();
        app.on_backend_event(BackendEvent::StreamUpdated(stream(
            40,
            "spotify",
            Some("mixlink.channel.music"),
        )));
        fake.take();
        let err = app
            .handle(Command::UnassignApp { app: "zzz".into() })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.options, vec!["spotify"]);
        app.handle(Command::UnassignApp { app: "spot".into() })
            .unwrap();
        assert_eq!(moves(&fake.take()), vec![(40, None)]);
        assert!(app.state().config.channel("music").unwrap().apps.is_empty());
    }
}
