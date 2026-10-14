//! Keeps a connection to mixlinkd, reconnecting forever, and forwards events to a sink.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Clone)]
pub struct DaemonLink {
    client: Arc<RwLock<Option<AsyncClient>>>,
    /// Whether meters are wanted (false while the window is hidden).
    meters: Arc<AtomicBool>,
}

impl Default for DaemonLink {
    fn default() -> Self {
        Self {
            client: Arc::default(),
            meters: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl DaemonLink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns the meter stream on or off; remembered across reconnects.
    pub async fn set_meters(&self, enabled: bool) {
        if self.meters.swap(enabled, Ordering::SeqCst) == enabled {
            return;
        }
        tracing::debug!("meters {}", if enabled { "on" } else { "off" });
        let client = self.client.read().await.clone();
        if let Some(client) = client
            && let Err(e) = client
                .call(Command::Subscribe {
                    state: true,
                    meters: enabled,
                })
                .await
        {
            tracing::debug!("could not change the meter subscription: {e}");
        }
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
                Ok((client, mut events)) => match initial_state(&client, &self.meters).await {
                    Ok((state, subscribed_meters)) => {
                        sink.state(&state);
                        *self.client.write().await = Some(client.clone());
                        // set_meters() during the handshake could not reach the daemon yet.
                        let wanted = self.meters.load(Ordering::SeqCst);
                        if wanted != subscribed_meters {
                            let _ = client
                                .call(Command::Subscribe {
                                    state: true,
                                    meters: wanted,
                                })
                                .await;
                        }
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

/// Subscribes (with the remembered meter flag) and loads the full state.
async fn initial_state(
    client: &AsyncClient,
    meters: &AtomicBool,
) -> Result<(State, bool), ClientError> {
    let meters = meters.load(Ordering::SeqCst);
    client
        .call(Command::Subscribe {
            state: true,
            meters,
        })
        .await?;
    let value = client.call(Command::GetState).await?;
    let state = serde_json::from_value(value).map_err(|e| ClientError::Protocol(e.to_string()))?;
    Ok((state, meters))
}
