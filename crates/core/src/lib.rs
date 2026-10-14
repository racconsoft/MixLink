//! Shared types and logic for mixlinkd, the CLI, the GUI and the MCP server.
#[cfg(feature = "tokio")]
pub mod async_client;
pub mod client;
pub mod model;
pub mod paths;
pub mod protocol;
pub mod resolve;
pub mod rules;
pub mod volume;
