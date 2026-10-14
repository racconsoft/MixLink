//! Loading, validating and atomically saving the TOML configuration.
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use wavelink_core::model::{Channel, Config, MixKind};

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
    Recovered { config: Config, backup: PathBuf, error: String },
}

pub fn load(path: &Path) -> LoadOutcome {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return LoadOutcome::Created(default_config()),
        Err(e) => return recover(path, e.to_string()),
    };
    match toml::from_str::<Config>(&text).map_err(|e| e.to_string()).and_then(validate) {
        Ok(config) => LoadOutcome::Loaded(config),
        Err(error) => recover(path, error),
    }
}

fn validate(config: Config) -> Result<Config, String> {
    if config.channels.is_empty() {
        return Err("config has no channels".into());
    }
    if config.channel(&config.fallback_channel).is_none() {
        return Err(format!("fallback channel '{}' does not exist", config.fallback_channel));
    }
    let mut ids = HashSet::new();
    if let Some(dup) = config.channels.iter().find(|c| !ids.insert(c.id.as_str())) {
        return Err(format!("duplicate channel id '{}'", dup.id));
    }
    Ok(config)
}

fn recover(path: &Path, error: String) -> LoadOutcome {
    let backup = path.with_extension("toml.bak");
    if let Err(e) = fs::copy(path, &backup) {
        tracing::warn!("could not back up {}: {e}", path.display());
    }
    LoadOutcome::Recovered { config: default_config(), backup, error }
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
                assert_eq!(ids, vec!["system", "game", "browser", "voice-chat", "music"]);
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
            LoadOutcome::Recovered { config, backup, error } => {
                assert_eq!(config, default_config());
                assert_eq!(std::fs::read_to_string(backup).unwrap(), "this is = = not toml");
                assert!(!error.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn missing_fallback_channel_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "fallback_channel = \"ghost\"\n[[channels]]\nid = \"a\"\nname = \"A\"\n").unwrap();
        assert!(matches!(load(&path), LoadOutcome::Recovered { .. }));
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
