//! Loading, validating and atomically saving the TOML configuration.
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use wavelink_core::model::{Channel, ChannelKind, Config, MixKind};

pub fn default_config() -> Config {
    let channel = |id: &str, name: &str, icon: &str, color: &str| {
        let mut c = Channel::new(id, name);
        c.icon = icon.into();
        c.color = color.into();
        c
    };
    Config {
        version: 1,
        fallback_channel: "system".into(),
        listen: MixKind::Monitor,
        settings: Default::default(),
        mixes: Default::default(),
        channels: vec![
            channel("system", "System", "system", "#64748b"),
            channel("game", "Game", "game", "#ef4444"),
            channel("browser", "Browser", "browser", "#a855f7"),
            channel("voice-chat", "Voice Chat", "chat", "#facc15"),
            channel("music", "Music", "music", "#ec4899"),
        ],
    }
}

#[derive(Debug)]
pub enum LoadOutcome {
    Loaded(Config),
    /// No config existed; defaults were created (first run).
    Created(Config),
    /// The file was unusable; it was copied to `backup` and defaults are used.
    Recovered {
        config: Config,
        backup: PathBuf,
        error: String,
    },
}

pub fn load(path: &Path) -> LoadOutcome {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return LoadOutcome::Created(default_config());
        }
        Err(e) => return recover(path, e.to_string()),
    };
    match toml::from_str::<Config>(&text)
        .map_err(|e| e.to_string())
        .and_then(validate)
    {
        Ok(config) => LoadOutcome::Loaded(config),
        Err(error) => recover(path, error),
    }
}

fn validate(config: Config) -> Result<Config, String> {
    if config.channels.is_empty() {
        return Err("config has no channels".into());
    }
    match config.channel(&config.fallback_channel) {
        None => {
            return Err(format!(
                "fallback channel '{}' does not exist",
                config.fallback_channel
            ));
        }
        Some(c) if c.kind == ChannelKind::Input => {
            return Err(format!(
                "fallback channel '{}' is an input channel",
                config.fallback_channel
            ));
        }
        Some(_) => {}
    }
    if let Some(bad) = config.channels.iter().find(|c| !is_slug(&c.id)) {
        return Err(format!(
            "channel id '{}' may only contain a-z, 0-9 and '-'",
            bad.id
        ));
    }
    let mut ids = HashSet::new();
    if let Some(dup) = config.channels.iter().find(|c| !ids.insert(c.id.as_str())) {
        return Err(format!("duplicate channel id '{}'", dup.id));
    }
    for c in &config.channels {
        for (mix, level) in [("monitor", &c.monitor), ("stream", &c.stream)] {
            if level.volume > 100 {
                return Err(format!(
                    "channel '{}': {mix} volume {} is above 100",
                    c.id, level.volume
                ));
            }
        }
    }
    for (mix, m) in [
        ("monitor", &config.mixes.monitor),
        ("stream", &config.mixes.stream),
    ] {
        if m.volume > 100 {
            return Err(format!("{mix} mix volume {} is above 100", m.volume));
        }
    }
    Ok(config)
}

/// Channel ids end up in PipeWire node names: `[a-z0-9-]+`.
fn is_slug(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn recover(path: &Path, error: String) -> LoadOutcome {
    let backup = path.with_extension("toml.bak");
    if let Err(e) = fs::copy(path, &backup) {
        tracing::warn!("could not back up {}: {e}", path.display());
    }
    LoadOutcome::Recovered {
        config: default_config(),
        backup,
        error,
    }
}

/// Writes the config atomically (temp file + rename).
pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(config).map_err(io::Error::other)?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_creates_defaults() {
        let dir = tempfile::tempdir().unwrap();
        match load(&dir.path().join("config.toml")) {
            LoadOutcome::Created(c) => {
                assert_eq!(c.fallback_channel, "system");
                let ids: Vec<&str> = c.channels.iter().map(|c| c.id.as_str()).collect();
                assert_eq!(
                    ids,
                    vec!["system", "game", "browser", "voice-chat", "music"]
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn save_then_load_roundtrips_and_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/wavelink-linux/config.toml");
        let mut config = default_config();
        config.channels[1].stream.volume = 42;
        save(&path, &config).unwrap();
        assert!(!path.with_extension("toml.tmp").exists());
        match load(&path) {
            LoadOutcome::Loaded(c) => assert_eq!(c, config),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_are_used() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is = = not toml").unwrap();
        match load(&path) {
            LoadOutcome::Recovered {
                config,
                backup,
                error,
            } => {
                assert_eq!(config, default_config());
                assert_eq!(
                    std::fs::read_to_string(backup).unwrap(),
                    "this is = = not toml"
                );
                assert!(!error.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn missing_fallback_channel_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "fallback_channel = \"ghost\"\n[[channels]]\nid = \"a\"\nname = \"A\"\n",
        )
        .unwrap();
        assert!(matches!(load(&path), LoadOutcome::Recovered { .. }));
    }

    fn load_text(text: &str) -> LoadOutcome {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        load(&path)
    }

    fn assert_invalid(text: &str, expected: &str) {
        match load_text(text) {
            LoadOutcome::Recovered { error, .. } => {
                assert!(
                    error.contains(expected),
                    "error '{error}' lacks '{expected}'"
                )
            }
            other => panic!("accepted invalid config {text:?}: {other:?}"),
        }
    }

    #[test]
    fn input_channel_as_fallback_is_invalid() {
        assert_invalid(
            "fallback_channel = \"mic\"\n[[channels]]\nid = \"mic\"\nname = \"Mic\"\nkind = \"input\"\nsource = \"alsa_input.x\"\n",
            "input channel",
        );
    }

    #[test]
    fn channel_ids_must_be_slugs() {
        for id in ["", "Music", "my music", "a.b", "nul\\u0000"] {
            assert_invalid(
                &format!(
                    "fallback_channel = \"a\"\n[[channels]]\nid = \"a\"\nname = \"A\"\n[[channels]]\nid = \"{id}\"\nname = \"B\"\n"
                ),
                "may only contain",
            );
        }
        assert!(matches!(
            load_text(
                "fallback_channel = \"voice-chat-2\"\n[[channels]]\nid = \"voice-chat-2\"\nname = \"V\"\n"
            ),
            LoadOutcome::Loaded(_)
        ));
    }

    #[test]
    fn volumes_above_100_are_invalid() {
        let base = "fallback_channel = \"a\"\n[[channels]]\nid = \"a\"\nname = \"A\"\n";
        assert_invalid(
            &format!("{base}monitor = {{ volume = 101 }}\n"),
            "monitor volume 101",
        );
        assert_invalid(
            &format!("{base}stream = {{ volume = 255 }}\n"),
            "stream volume 255",
        );
        assert_invalid(
            &format!("{base}[mixes.stream]\nvolume = 150\n"),
            "stream mix volume 150",
        );
        assert!(matches!(
            load_text(&format!(
                "{base}stream = {{ volume = 0 }}\n[mixes.monitor]\nvolume = 100\n"
            )),
            LoadOutcome::Loaded(_)
        ));
    }

    #[test]
    fn unknown_tables_and_missing_fields_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "fallback_channel = \"a\"\n[[channels]]\nid = \"a\"\nname = \"A\"\n[mic_fx.rnnoise]\nenabled = true\n",
        )
        .unwrap();
        match load(&path) {
            LoadOutcome::Loaded(c) => assert_eq!(c.channels[0].stream.volume, 100),
            other => panic!("unexpected {other:?}"),
        }
    }
}
