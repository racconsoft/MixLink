//! Keeps a connection to mixlinkd, reconnecting forever, and forwards events to a sink.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mixlink_core::async_client::AsyncClient;
use mixlink_core::client::ClientError;
use mixlink_core::model::State;
use mixlink_core::protocol::{Command, ErrorBody, ErrorCode, Event};
use serde_json::Value;
use tokio::sync::RwLock;

pub const NOT_RUNNING: &str = "MixLink service is not running";

/// Receives everything the GUI needs to render.
pub trait LinkSink: Send + Sync + 'static {
    fn state(&self, state: &State);
    /// Always an `Event::Meters`.
    fn meters(&self, meters: &Event);
    fn connection(&self, connected: bool);
}

#[derive(Clone, Default)]
pub struct DaemonLink {
    client: Arc<RwLock<Option<AsyncClient>>>,
}

impl DaemonLink {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn send(&self, command: Command) -> Result<Value, ErrorBody> {
        let client = self.client.read().await.clone();
        let Some(client) = client else {
            return Err(ErrorBody::new(ErrorCode::Internal, NOT_RUNNING));
        };
        client.call(command).await.map_err(|e| match e {
            ClientError::Daemon(body) => body,
            other => ErrorBody::new(ErrorCode::Internal, other.to_string()),
        })
    }

    /// Connects, subscribes and forwards events; on loss waits `retry` and starts over.
    pub async fn run(self, socket: PathBuf, sink: Arc<dyn LinkSink>, retry: Duration) {
        let mut reported: Option<bool> = None;
        let mut report = |connected: bool, sink: &Arc<dyn LinkSink>| {
            if reported != Some(connected) {
                reported = Some(connected);
                sink.connection(connected);
            }
        };
        loop {
            match AsyncClient::connect(&socket).await {
                Err(e) => tracing::debug!("could not connect to mixlinkd: {e}"),
                Ok((client, mut events)) => match initial_state(&client).await {
                    Ok(state) => {
                        sink.state(&state);
                        *self.client.write().await = Some(client);
                        report(true, &sink);
                        while let Some(event) = events.next().await {
                            match &event {
                                Event::StateChanged { state } => sink.state(state),
                                Event::Meters { .. } => sink.meters(&event),
                            }
                        }
                        *self.client.write().await = None;
                        tracing::info!("lost the connection to mixlinkd");
                    }
                    Err(e) => tracing::warn!("mixlinkd handshake failed: {e}"),
                },
            }
            report(false, &sink);
            tokio::time::sleep(retry).await;
        }
    }
}

async fn initial_state(client: &AsyncClient) -> Result<State, ClientError> {
    client
        .call(Command::Subscribe {
            state: true,
            meters: true,
        })
        .await?;
    let value = client.call(Command::GetState).await?;
    serde_json::from_value(value).map_err(|e| ClientError::Protocol(e.to_string()))
}
