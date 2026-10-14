//! The boundary between the state machine and the audio system.
pub mod fake;

use wavelink_core::model::{ChannelId, Device};

use crate::mixer::dsp::{Peaks, Targets};

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelSource {
    /// A virtual sink apps can play into.
    Virtual,
    /// An existing capture device (by `node.name`).
    Input { source: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackendCommand {
    /// Create or update the channel bound to mixer slot `slot`.
    EnsureChannel {
        id: ChannelId,
        slot: usize,
        description: String,
        source: ChannelSource,
    },
    RemoveChannel {
        id: ChannelId,
    },
    SetTargets(Targets),
    /// `node.name` of the playback device for the monitor mix.
    SetMonitorOutput(Option<String>),
    /// `node.name` to configure as default sink; `None` clears the setting.
    SetDefaultSink(Option<String>),
    /// Move a stream to the node named `target` (deferred until it exists).
    /// `None` clears the stream's `target.object`, so it follows the default sink again.
    MoveStream {
        node_id: u32,
        target: Option<String>,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamInfo {
    pub node_id: u32,
    pub app: String,
    pub name: String,
    pub icon: Option<String>,
    /// `node.name` the stream is currently linked to.
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    Ready {
        previous_default_sink: Option<String>,
    },
    /// A new stream or a change to a known one.
    StreamUpdated(StreamInfo),
    StreamRemoved(u32),
    DeviceAdded(Device),
    DeviceRemoved(String),
    Meters(Peaks),
    Disconnected,
}

pub trait AudioBackend: Send + 'static {
    fn send(&self, cmd: BackendCommand);
}
