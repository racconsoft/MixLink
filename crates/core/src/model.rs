//! Domain types shared by daemon, CLI, GUI and MCP server.
use serde::{Deserialize, Serialize};

pub const MAX_CHANNELS: usize = 16;
/// Every PipeWire node the daemon owns starts with this prefix.
pub const NODE_PREFIX: &str = "wavelink.";
const CHANNEL_NODE_PREFIX: &str = "wavelink.channel.";

pub type ChannelId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MixKind {
    Monitor,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MixTarget {
    Monitor,
    Stream,
    Both,
}

impl MixTarget {
    pub fn kinds(self) -> &'static [MixKind] {
        match self {
            Self::Monitor => &[MixKind::Monitor],
            Self::Stream => &[MixKind::Stream],
            Self::Both => &[MixKind::Monitor, MixKind::Stream],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Level {
    pub volume: u8,
    pub muted: bool,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            volume: 100,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    #[default]
    Virtual,
    Input,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub name: String,
    #[serde(default)]
    pub kind: ChannelKind,
    /// PipeWire node name of the capture device for `ChannelKind::Input`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub linked: bool,
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub monitor: Level,
    #[serde(default)]
    pub stream: Level,
}

fn default_icon() -> String {
    "default".into()
}

fn default_color() -> String {
    "#6b7280".into()
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: ChannelKind::Virtual,
            source: None,
            icon: default_icon(),
            color: default_color(),
            linked: false,
            apps: Vec::new(),
            monitor: Level::default(),
            stream: Level::default(),
        }
    }

    pub fn node_name(&self) -> String {
        channel_node_name(&self.id)
    }

    pub fn level(&self, mix: MixKind) -> &Level {
        match mix {
            MixKind::Monitor => &self.monitor,
            MixKind::Stream => &self.stream,
        }
    }

    pub fn level_mut(&mut self, mix: MixKind) -> &mut Level {
        match mix {
            MixKind::Monitor => &mut self.monitor,
            MixKind::Stream => &mut self.stream,
        }
    }
}

pub fn channel_node_name(id: &str) -> String {
    format!("{CHANNEL_NODE_PREFIX}{id}")
}

pub fn channel_id_from_node_name(name: &str) -> Option<&str> {
    name.strip_prefix(CHANNEL_NODE_PREFIX)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mix {
    pub volume: u8,
    pub muted: bool,
    /// PipeWire node name of the playback device (monitor mix only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_device: Option<String>,
}

impl Default for Mix {
    fn default() -> Self {
        Self {
            volume: 100,
            muted: false,
            output_device: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mixes {
    pub monitor: Mix,
    pub stream: Mix,
}

impl Mixes {
    pub fn get(&self, mix: MixKind) -> &Mix {
        match mix {
            MixKind::Monitor => &self.monitor,
            MixKind::Stream => &self.stream,
        }
    }

    pub fn get_mut(&mut self, mix: MixKind) -> &mut Mix {
        match mix {
            MixKind::Monitor => &mut self.monitor,
            MixKind::Stream => &mut self.stream,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_xlr_profile: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_xlr_profile: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    pub fallback_channel: ChannelId,
    #[serde(default = "default_listen")]
    pub listen: MixKind,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub mixes: Mixes,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

fn default_version() -> u32 {
    1
}

fn default_listen() -> MixKind {
    MixKind::Monitor
}

impl Config {
    pub fn channel(&self, id: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn channel_mut(&mut self, id: &str) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|c| c.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Sink,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// PipeWire `node.name`.
    pub name: String,
    pub description: String,
    pub kind: DeviceKind,
}

/// A running application audio stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveApp {
    pub node_id: u32,
    /// Normalized identity used by assignment rules (e.g. "firefox").
    pub app: String,
    /// Display name (e.g. "Firefox").
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Channel the stream currently plays into, if it is one of ours.
    #[serde(default)]
    pub channel: Option<ChannelId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub config: Config,
    #[serde(default)]
    pub apps: Vec<ActiveApp>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_EXAMPLE: &str = r##"
version = 1
fallback_channel = "system"
listen = "monitor"

[settings]
auto_xlr_profile = true

[mixes.monitor]
output_device = "alsa_output.usb-Elgato_Systems_Elgato_Wave_XLR_0000000000-00.analog-stereo"
volume = 100
muted = false

[mixes.stream]
volume = 80
muted = false

[[channels]]
id = "system"
name = "System"

[[channels]]
id = "voice-chat"
name = "Voice Chat"
kind = "virtual"
icon = "chat"
color = "#facc15"
monitor = { volume = 100, muted = false }
stream  = { volume = 60, muted = true }
linked = false
apps = ["discord", "slack"]

[mic_fx.rnnoise]
enabled = true
"##;

    #[test]
    fn parses_spec_example_and_ignores_unknown_tables() {
        let config: Config = toml::from_str(SPEC_EXAMPLE).unwrap();
        assert_eq!(config.fallback_channel, "system");
        assert_eq!(config.mixes.stream.volume, 80);
        assert_eq!(config.channels.len(), 2);
        let voice = config.channel("voice-chat").unwrap();
        assert_eq!(
            voice.stream,
            Level {
                volume: 60,
                muted: true
            }
        );
        assert_eq!(voice.apps, vec!["discord", "slack"]);
    }

    #[test]
    fn missing_channel_fields_get_defaults() {
        let config: Config = toml::from_str(SPEC_EXAMPLE).unwrap();
        let system = config.channel("system").unwrap();
        assert_eq!(system.kind, ChannelKind::Virtual);
        assert_eq!(
            system.monitor,
            Level {
                volume: 100,
                muted: false
            }
        );
        assert_eq!(system.icon, "default");
        assert!(system.apps.is_empty());
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let config: Config = toml::from_str(SPEC_EXAMPLE).unwrap();
        let text = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn mix_target_expands_to_kinds() {
        assert_eq!(
            MixTarget::Both.kinds(),
            &[MixKind::Monitor, MixKind::Stream]
        );
        assert_eq!(MixTarget::Stream.kinds(), &[MixKind::Stream]);
    }

    #[test]
    fn node_names_roundtrip() {
        assert_eq!(channel_node_name("music"), "wavelink.channel.music");
        assert_eq!(
            channel_id_from_node_name("wavelink.channel.music"),
            Some("music")
        );
        assert_eq!(channel_id_from_node_name("alsa_output.foo"), None);
    }
}
