//! Unix socket server: one task per client, JSON lines in both directions.
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, oneshot};
use wavelink_core::protocol::{Command, ErrorBody, ErrorCode, Request, Response, to_line};

use crate::actor::{ActorMsg, Broadcast};

/// Binds the socket. Refuses if another daemon answers on it; removes stale files.
pub fn bind_socket(path: &Path) -> io::Result<UnixListener> {
    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("wavelinkd is already running ({})", path.display()),
            ));
        }
        std::fs::remove_file(path)?;
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

pub async fn serve(
    listener: UnixListener,
    actor: mpsc::Sender<ActorMsg>,
    broadcast: broadcast::Sender<Broadcast>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_client(stream, actor.clone(), broadcast.clone()));
            }
            Err(e) => tracing::warn!("accepting a client failed: {e}"),
        }
    }
}

async fn handle_client(
    stream: UnixStream,
    actor: mpsc::Sender<ActorMsg>,
    broadcast: broadcast::Sender<Broadcast>,
) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let mut events: Option<broadcast::Receiver<Broadcast>> = None;
    let (mut want_state, mut want_meters) = (false, false);
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let response = match serde_json::from_str::<Request>(&line) {
                    Err(e) => {
                        let id = serde_json::from_str::<serde_json::Value>(&line)
                            .ok()
                            .and_then(|v| v.get("id").and_then(|id| id.as_u64()))
                            .unwrap_or(0);
                        Response::err(id, ErrorBody::invalid(format!("invalid request: {e}")))
                    }
                    Ok(request) => {
                        if let Command::Subscribe { state, meters } = request.command {
                            want_state = state;
                            want_meters = meters;
                            events = if state || meters { Some(broadcast.subscribe()) } else { None };
                        }
                        dispatch(&actor, request).await
                    }
                };
                if write.write_all(to_line(&response).as_bytes()).await.is_err() {
                    break;
                }
            }
            message = next_broadcast(&mut events) => {
                let line = match message {
                    Ok(Broadcast::State(line)) if want_state => line,
                    Ok(Broadcast::Meters(line)) if want_meters => line,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        events = None;
                        continue;
                    }
                };
                if write.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn next_broadcast(
    events: &mut Option<broadcast::Receiver<Broadcast>>,
) -> Result<Broadcast, broadcast::error::RecvError> {
    match events {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn dispatch(actor: &mpsc::Sender<ActorMsg>, request: Request) -> Response {
    let shutting_down = || ErrorBody::new(ErrorCode::Internal, "wavelinkd is shutting down");
    let (reply, rx) = oneshot::channel();
    if actor
        .send(ActorMsg::Command {
            command: request.command,
            reply,
        })
        .await
        .is_err()
    {
        return Response::err(request.id, shutting_down());
    }
    match rx.await {
        Ok(Ok(value)) => Response::ok(request.id, value),
        Ok(Err(error)) => Response::err(request.id, error),
        Err(_) => Response::err(request.id, shutting_down()),
    }
}
