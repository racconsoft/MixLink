//! Line-based JSON protocol between wavelinkd and its clients.
use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ChannelId, MixKind, MixTarget, State};
use crate::resolve::ResolveError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum Command {
    GetState,
    Subscribe {
        #[serde(default)]
        state: bool,
        #[serde(default)]
        meters: bool,
    },
    ListApps,
    ListDevices,
    SetVolume {
        channel: String,
        mix: MixTarget,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<i32>,
    },
    /// `muted: None` toggles.
    SetMute {
        channel: String,
        mix: MixTarget,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        muted: Option<bool>,
    },
    SetLink { channel: String, linked: bool },
    AssignApp { app: String, channel: String },
    UnassignApp { app: String },
    CreateChannel {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    RenameChannel { channel: String, name: String },
    UpdateChannel {
        channel: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    DeleteChannel { channel: String },
    ReorderChannels { order: Vec<String> },
    SetMix {
        mix: MixKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        volume: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        muted: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_device: Option<String>,
    },
    SetListen { mix: MixKind },
    SetFallbackChannel { channel: String },
    SetSettings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auto_xlr_profile: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    Ambiguous,
    InvalidArgument,
    LimitReached,
    NotAllowed,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

impl ErrorBody {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), options: Vec::new() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }
}

impl From<ResolveError> for ErrorBody {
    fn from(e: ResolveError) -> Self {
        let code = match e {
            ResolveError::NotFound { .. } => ErrorCode::NotFound,
            ResolveError::Ambiguous { .. } => ErrorCode::Ambiguous,
        };
        Self { code, message: e.to_string(), options: e.options().to_vec() }
    }
}

impl fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if !self.options.is_empty() {
            write!(f, " (options: {})", self.options.join(", "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self { id, ok: true, result: Some(result), error: None }
    }

    pub fn err(id: u64, error: ErrorBody) -> Self {
        Self { id, ok: false, result: None, error: Some(error) }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    StateChanged {
        state: State,
    },
    Meters {
        channels: BTreeMap<ChannelId, [f32; 2]>,
        monitor: [f32; 2],
        stream: [f32; 2],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Response(Response),
    Event(Event),
}

/// Serializes one protocol message as a JSON line.
pub fn to_line<T: Serialize>(value: &T) -> String {
    let mut line = serde_json::to_string(value).expect("protocol messages are serializable");
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ResolveError;

    #[test]
    fn parses_request_with_args() {
        let req: Request = serde_json::from_str(
            r#"{"id":7,"cmd":"set_volume","args":{"channel":"music","mix":"stream","delta":-10}}"#,
        )
        .unwrap();
        assert_eq!(req.id, 7);
        assert_eq!(
            req.command,
            Command::SetVolume { channel: "music".into(), mix: MixTarget::Stream, value: None, delta: Some(-10) }
        );
    }

    #[test]
    fn unit_commands_need_no_args() {
        let req: Request = serde_json::from_str(r#"{"id":1,"cmd":"get_state"}"#).unwrap();
        assert_eq!(req.command, Command::GetState);
        let line = to_line(&Request { id: 2, command: Command::ListApps });
        assert!(line.contains(r#""cmd":"list_apps""#));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn request_roundtrips() {
        let req = Request {
            id: 3,
            command: Command::SetMix {
                mix: MixKind::Monitor,
                volume: Some(80),
                delta: None,
                muted: Some(false),
                output_device: Some("alsa_output.x".into()),
            },
        };
        let back: Request = serde_json::from_str(&to_line(&req)).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn error_response_carries_code_and_options() {
        let body: ErrorBody = ResolveError::NotFound { query: "xyz".into(), options: vec!["Music".into()] }.into();
        let line = to_line(&Response::err(4, body));
        assert!(line.contains(r#""code":"not_found""#));
        assert!(line.contains(r#""options":["Music"]"#));
        assert!(!line.contains("result"));
    }

    #[test]
    fn error_display_lists_options() {
        let body = ErrorBody {
            code: ErrorCode::Ambiguous,
            message: "'gam' is ambiguous".into(),
            options: vec!["Game".into(), "Game Capture".into()],
        };
        assert_eq!(body.to_string(), "'gam' is ambiguous (options: Game, Game Capture)");
    }

    #[test]
    fn server_messages_are_distinguished() {
        let ev: ServerMessage =
            serde_json::from_str(r#"{"event":"meters","channels":{"music":[0.5,0.25]},"monitor":[0,0],"stream":[0,0]}"#)
                .unwrap();
        assert!(matches!(ev, ServerMessage::Event(Event::Meters { .. })));
        let resp: ServerMessage = serde_json::from_str(r#"{"id":1,"ok":true,"result":null}"#).unwrap();
        assert!(matches!(resp, ServerMessage::Response(Response { id: 1, ok: true, .. })));
    }
}
