//! Human-readable output and argument parsing for the CLI.
use std::fmt::Write;

use wavelink_core::model::{Channel, Level, MixKind, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelArg {
    Set(u8),
    Delta(i32),
}

pub fn parse_level(input: &str) -> Result<LevelArg, String> {
    let s = input.trim();
    let bad = || format!("invalid level '{s}': use 0–100, +N or -N");
    if let Some(rest) = s.strip_prefix('+') {
        return rest.parse::<i32>().map(LevelArg::Delta).map_err(|_| bad());
    }
    if s.starts_with('-') {
        return s.parse::<i32>().map(LevelArg::Delta).map_err(|_| bad());
    }
    match s.parse::<u8>() {
        Ok(v) if v <= 100 => Ok(LevelArg::Set(v)),
        _ => Err(bad()),
    }
}

fn level(l: &Level) -> String {
    if l.muted { format!("{} (muted)", l.volume) } else { l.volume.to_string() }
}

pub fn format_channel(c: &Channel) -> String {
    format!("{}: monitor {}, stream {}", c.name, level(&c.monitor), level(&c.stream))
}

pub fn format_status(state: &State) -> String {
    let config = &state.config;
    let mut out = String::new();
    let _ = writeln!(out, "{:<18} {:<12} {:<12} APPS", "CHANNEL", "MONITOR", "STREAM");
    for c in &config.channels {
        let name = if c.id == config.fallback_channel { format!("{}*", c.name) } else { c.name.clone() };
        let _ = writeln!(out, "{:<18} {:<12} {:<12} {}", name, level(&c.monitor), level(&c.stream), c.apps.join(", "));
    }
    let _ = writeln!(out);
    let monitor = &config.mixes.monitor;
    let stream = &config.mixes.stream;
    let device = monitor.output_device.as_deref().unwrap_or("(none)");
    let mute = |m: bool| if m { " (muted)" } else { "" };
    let _ = writeln!(out, "Monitor mix: {}{} → {}", monitor.volume, mute(monitor.muted), device);
    let _ = writeln!(out, "Stream mix:  {}{} → WaveLink Stream Mix", stream.volume, mute(stream.muted));
    let listen = match config.listen {
        MixKind::Monitor => "monitor",
        MixKind::Stream => "stream",
    };
    let _ = writeln!(out, "Listening to: {listen}");
    if !state.apps.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "RUNNING APPS");
        for app in &state.apps {
            let channel = app
                .channel
                .as_deref()
                .and_then(|id| config.channel(id))
                .map(|c| c.name.as_str())
                .unwrap_or("(not in a WaveLink channel)");
            let _ = writeln!(out, "  {} → {}", app.name, channel);
        }
    }
    for warning in &state.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wavelink_core::model::{ActiveApp, Channel, Config, MixKind};

    fn state() -> State {
        let mut music = Channel::new("music", "Music");
        music.apps = vec!["spotify".into()];
        music.stream.volume = 40;
        music.stream.muted = true;
        let mut config = Config {
            version: 1,
            fallback_channel: "system".into(),
            listen: MixKind::Monitor,
            settings: Default::default(),
            mixes: Default::default(),
            channels: vec![Channel::new("system", "System"), music],
        };
        config.mixes.monitor.output_device = Some("alsa_output.headphones".into());
        State {
            config,
            apps: vec![ActiveApp {
                node_id: 1,
                app: "spotify".into(),
                name: "Spotify".into(),
                icon: None,
                channel: Some("music".into()),
            }],
            devices: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn parses_levels() {
        assert_eq!(parse_level("40"), Ok(LevelArg::Set(40)));
        assert_eq!(parse_level("+10"), Ok(LevelArg::Delta(10)));
        assert_eq!(parse_level("-15"), Ok(LevelArg::Delta(-15)));
        assert!(parse_level("150").is_err());
        assert!(parse_level("loud").is_err());
    }

    #[test]
    fn status_lists_channels_mixes_and_apps() {
        let text = format_status(&state());
        assert!(text.contains("System*"), "fallback is marked:\n{text}");
        assert!(text.contains("40 (muted)"));
        assert!(text.contains("spotify"));
        assert!(text.contains("alsa_output.headphones"));
        assert!(text.contains("Spotify → Music"));
        assert!(text.contains("Listening to: monitor"));
    }

    #[test]
    fn channel_summary() {
        let s = state();
        assert_eq!(format_channel(&s.config.channels[1]), "Music: monitor 100, stream 40 (muted)");
    }
}
