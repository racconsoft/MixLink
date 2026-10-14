//! Human-readable output and argument parsing for the CLI.
use std::fmt::Write;

use mixlink_core::model::{Channel, Device, DeviceKind, Level, Mix, MixKind, State};

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

/// Argument of `mixlink mix`: a level or a mute action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixLevelArg {
    Level(LevelArg),
    Mute,
    Unmute,
    Toggle,
}

pub fn parse_mix_level(input: &str) -> Result<MixLevelArg, String> {
    match input.trim().to_lowercase().as_str() {
        "mute" => Ok(MixLevelArg::Mute),
        "unmute" => Ok(MixLevelArg::Unmute),
        "toggle" => Ok(MixLevelArg::Toggle),
        _ => parse_level(input)
            .map(MixLevelArg::Level)
            .map_err(|e| format!("{e}, mute, unmute or toggle")),
    }
}

/// Where the stream mix goes: just OBS, or OBS plus a hardware device.
fn stream_target(mix: &Mix) -> String {
    match mix.output_device.as_deref() {
        Some(device) => format!("MixLink Stream Mix (OBS) + {device}"),
        None => "MixLink Stream Mix (OBS only)".to_string(),
    }
}

/// `Monitor mix: 80 → alsa_output.x` / `Stream mix: 60 (muted) → MixLink Stream Mix (OBS only)`
pub fn format_mix(kind: MixKind, mix: &Mix) -> String {
    let (label, output) = match kind {
        MixKind::Monitor => (
            "Monitor",
            mix.output_device.as_deref().unwrap_or("(none)").to_string(),
        ),
        MixKind::Stream => ("Stream", stream_target(mix)),
    };
    let muted = if mix.muted { " (muted)" } else { "" };
    format!("{label} mix: {}{muted} → {output}", mix.volume)
}

/// Playback devices; `M` marks the current monitor output, `S` the stream output.
pub fn format_outputs(devices: &[Device], monitor: Option<&str>, stream: Option<&str>) -> String {
    let sinks: Vec<&Device> = devices
        .iter()
        .filter(|d| d.kind == DeviceKind::Sink)
        .collect();
    if sinks.is_empty() {
        return "No playback devices found.\n".into();
    }
    let width = sinks
        .iter()
        .map(|d| d.description.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for d in sinks {
        let m = if monitor == Some(d.name.as_str()) {
            'M'
        } else {
            ' '
        };
        let s = if stream == Some(d.name.as_str()) {
            'S'
        } else {
            ' '
        };
        let _ = writeln!(out, "{m}{s} {:<width$}  {}", d.description, d.name);
    }
    out
}

fn level(l: &Level) -> String {
    if l.muted {
        format!("{} (muted)", l.volume)
    } else {
        l.volume.to_string()
    }
}

pub fn format_channel(c: &Channel) -> String {
    format!(
        "{}: monitor {}, stream {}",
        c.name,
        level(&c.monitor),
        level(&c.stream)
    )
}

pub fn format_status(state: &State) -> String {
    let config = &state.config;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<18} {:<12} {:<12} APPS",
        "CHANNEL", "MONITOR", "STREAM"
    );
    for c in &config.channels {
        let name = if c.id == config.fallback_channel {
            format!("{}*", c.name)
        } else {
            c.name.clone()
        };
        let _ = writeln!(
            out,
            "{:<18} {:<12} {:<12} {}",
            name,
            level(&c.monitor),
            level(&c.stream),
            c.apps.join(", ")
        );
    }
    let _ = writeln!(out);
    let monitor = &config.mixes.monitor;
    let stream = &config.mixes.stream;
    let device = monitor.output_device.as_deref().unwrap_or("(none)");
    let mute = |m: bool| if m { " (muted)" } else { "" };
    let _ = writeln!(
        out,
        "Monitor mix: {}{} → {}",
        monitor.volume,
        mute(monitor.muted),
        device
    );
    let _ = writeln!(
        out,
        "Stream mix:  {}{} → {}",
        stream.volume,
        mute(stream.muted),
        stream_target(stream)
    );
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
                .unwrap_or("(not in a MixLink channel)");
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
    use mixlink_core::model::{ActiveApp, Channel, Config, MixKind};

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
        assert!(
            text.contains("MixLink Stream Mix (OBS only)"),
            "no stream output device is configured:\n{text}"
        );
    }

    #[test]
    fn status_shows_the_stream_output_device_when_set() {
        let mut s = state();
        s.config.mixes.stream.output_device = Some("alsa_output.hdmi".into());
        let text = format_status(&s);
        assert!(text.contains("MixLink Stream Mix (OBS) + alsa_output.hdmi"));
    }

    #[test]
    fn parses_mix_levels() {
        assert_eq!(
            parse_mix_level("80"),
            Ok(MixLevelArg::Level(LevelArg::Set(80)))
        );
        assert_eq!(
            parse_mix_level("-5"),
            Ok(MixLevelArg::Level(LevelArg::Delta(-5)))
        );
        assert_eq!(parse_mix_level("Mute"), Ok(MixLevelArg::Mute));
        assert_eq!(parse_mix_level("unmute"), Ok(MixLevelArg::Unmute));
        assert_eq!(parse_mix_level("toggle"), Ok(MixLevelArg::Toggle));
        let err = parse_mix_level("loud").unwrap_err();
        assert!(err.contains("toggle"), "{err}");
    }

    #[test]
    fn mix_summary() {
        let s = state();
        assert_eq!(
            format_mix(MixKind::Monitor, &s.config.mixes.monitor),
            "Monitor mix: 100 → alsa_output.headphones"
        );
        let stream = Mix {
            volume: 60,
            muted: true,
            output_device: None,
        };
        assert_eq!(
            format_mix(MixKind::Stream, &stream),
            "Stream mix: 60 (muted) → MixLink Stream Mix (OBS only)"
        );
        let stream_with_device = Mix {
            volume: 60,
            muted: false,
            output_device: Some("alsa_output.hdmi".into()),
        };
        assert_eq!(
            format_mix(MixKind::Stream, &stream_with_device),
            "Stream mix: 60 → MixLink Stream Mix (OBS) + alsa_output.hdmi"
        );
    }

    fn devices_fixture() -> Vec<Device> {
        let device = |name: &str, description: &str, kind| Device {
            name: name.into(),
            description: description.into(),
            kind,
        };
        vec![
            device("alsa_output.speakers", "Speakers", DeviceKind::Sink),
            device("alsa_input.mic", "Mic", DeviceKind::Source),
            device("alsa_output.hdmi", "HDMI Audio", DeviceKind::Sink),
        ]
    }

    #[test]
    fn outputs_mark_the_current_monitor_and_stream_devices() {
        let devices = devices_fixture();
        let text = format_outputs(&devices, Some("alsa_output.hdmi"), Some("alsa_output.hdmi"));
        assert_eq!(
            text,
            "   Speakers    alsa_output.speakers\nMS HDMI Audio  alsa_output.hdmi\n"
        );
        assert!(format_outputs(&[], None, None).contains("No playback devices"));
    }

    #[test]
    fn outputs_mark_monitor_and_stream_devices_separately() {
        let devices = devices_fixture();
        let text = format_outputs(
            &devices,
            Some("alsa_output.speakers"),
            Some("alsa_output.hdmi"),
        );
        assert_eq!(
            text,
            "M  Speakers    alsa_output.speakers\n S HDMI Audio  alsa_output.hdmi\n"
        );
    }

    #[test]
    fn channel_summary() {
        let s = state();
        assert_eq!(
            format_channel(&s.config.channels[1]),
            "Music: monitor 100, stream 40 (muted)"
        );
    }
}
