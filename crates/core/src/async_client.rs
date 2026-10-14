//! Async client for the mixlinkd socket (GUI, MCP server).
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot};

use crate::client::ClientError;
use crate::protocol::{Command, ErrorBody, ErrorCode, Event, Request, ServerMessage, to_line};

type Reply = oneshot::Sender<Result<Value, ClientError>>;

struct Shared {
    pending: Mutex<HashMap<u64, Reply>>,
    closed: AtomicBool,
    next_id: AtomicU64,
}

/// Sends commands; cheap to clone. Responses are matched to requests by id.
#[derive(Clone)]
pub struct AsyncClient {
    writer: Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
    shared: Arc<Shared>,
}

/// Events pushed by the daemon after a `subscribe` command.
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventStream {
    /// Next event; `None` once the connection is closed.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

fn closed_error() -> ClientError {
    ClientError::Io(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "mixlinkd closed the connection",
    ))
}

impl AsyncClient {
    pub async fn connect(path: &Path) -> Result<(AsyncClient, EventStream), ClientError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(|source| ClientError::Connect {
                path: path.to_path_buf(),
                source,
            })?;
        let (read, write) = stream.into_split();
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
        });
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(read, shared.clone(), tx));
        Ok((
            AsyncClient {
                writer: Arc::new(tokio::sync::Mutex::new(write)),
                shared,
            },
            EventStream { rx },
        ))
    }

    pub async fn call(&self, command: Command) -> Result<Value, ClientError> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.shared.pending.lock().unwrap().insert(id, tx);
        if self.shared.closed.load(Ordering::Acquire) {
            self.shared.pending.lock().unwrap().remove(&id);
            return Err(closed_error());
        }
        let line = to_line(&Request { id, command });
        if let Err(e) = self.writer.lock().await.write_all(line.as_bytes()).await {
            self.shared.pending.lock().unwrap().remove(&id);
            return Err(ClientError::Io(e));
        }
        rx.await.unwrap_or_else(|_| Err(closed_error()))
    }
}

async fn read_loop(read: OwnedReadHalf, shared: Arc<Shared>, events: mpsc::UnboundedSender<Event>) {
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<ServerMessage>(&line) {
            Ok(ServerMessage::Response(response)) => {
                let reply = shared.pending.lock().unwrap().remove(&response.id);
                if let Some(reply) = reply {
                    let result = if response.ok {
                        Ok(response.result.unwrap_or(Value::Null))
                    } else {
                        Err(ClientError::Daemon(response.error.unwrap_or_else(|| {
                            ErrorBody::new(ErrorCode::Internal, "error without details")
                        })))
                    };
                    let _ = reply.send(result);
                }
            }
            Ok(ServerMessage::Event(event)) => {
                let _ = events.send(event);
            }
            Err(_) => {} // ignore lines we do not understand
        }
    }
    shared.closed.store(true, Ordering::Release);
    let pending: Vec<Reply> = shared
        .pending
        .lock()
        .unwrap()
        .drain()
        .map(|(_, reply)| reply)
        .collect();
    for reply in pending {
        let _ = reply.send(Err(closed_error()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Request;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

    async fn server(dir: &tempfile::TempDir) -> (std::path::PathBuf, UnixListener) {
        let path = dir.path().join("d.sock");
        let listener = UnixListener::bind(&path).unwrap();
        (path, listener)
    }

    #[tokio::test]
    async fn call_returns_result_and_events_are_streamed() {
        let dir = tempfile::tempdir().unwrap();
        let (path, listener) = server(&dir).await;
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (r, mut w) = stream.into_split();
            let mut lines = BufReader::new(r).lines();
            let req: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            w.write_all(
                b"{\"event\":\"meters\",\"channels\":{},\"monitor\":[0.5,0.5],\"stream\":[0,0]}\n",
            )
            .await
            .unwrap();
            w.write_all(
                format!("{{\"id\":{},\"ok\":true,\"result\":{{\"x\":1}}}}\n", req.id).as_bytes(),
            )
            .await
            .unwrap();
            let _ = lines.next_line().await;
        });
        let (client, mut events) = AsyncClient::connect(&path).await.unwrap();
        let value = client.call(Command::GetState).await.unwrap();
        assert_eq!(value["x"], 1);
        match events.next().await {
            Some(Event::Meters { monitor, .. }) => assert_eq!(monitor, [0.5, 0.5]),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn concurrent_calls_are_matched_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let (path, listener) = server(&dir).await;
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (r, mut w) = stream.into_split();
            let mut lines = BufReader::new(r).lines();
            let a: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let b: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            // answer in reverse order
            for req in [&b, &a] {
                w.write_all(
                    format!("{{\"id\":{0},\"ok\":true,\"result\":{0}}}\n", req.id).as_bytes(),
                )
                .await
                .unwrap();
            }
            let _ = lines.next_line().await;
        });
        let (client, _events) = AsyncClient::connect(&path).await.unwrap();
        let (c1, c2) = (client.clone(), client.clone());
        let (r1, r2) = tokio::join!(c1.call(Command::GetState), c2.call(Command::ListApps));
        assert_ne!(r1.unwrap(), r2.unwrap());
    }

    #[tokio::test]
    async fn daemon_errors_and_closed_connections() {
        let dir = tempfile::tempdir().unwrap();
        let (path, listener) = server(&dir).await;
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (r, mut w) = stream.into_split();
            let mut lines = BufReader::new(r).lines();
            let req: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            w.write_all(
                format!("{{\"id\":{},\"ok\":false,\"error\":{{\"code\":\"not_found\",\"message\":\"nope\",\"options\":[\"Music\"]}}}}\n", req.id)
                    .as_bytes(),
            )
            .await
            .unwrap();
            let _ = lines.next_line().await; // read the second request, then hang up
        });
        let (client, mut events) = AsyncClient::connect(&path).await.unwrap();
        match client.call(Command::GetState).await {
            Err(ClientError::Daemon(body)) => assert_eq!(body.options, vec!["Music"]),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            client.call(Command::GetState).await,
            Err(ClientError::Io(_))
        ));
        assert!(
            events.next().await.is_none(),
            "stream ends when the daemon hangs up"
        );
    }

    #[tokio::test]
    async fn connect_error_names_the_socket() {
        let err = AsyncClient::connect(std::path::Path::new("/nonexistent/mixlink.sock"))
            .await
            .err()
            .unwrap();
        assert!(err.to_string().contains("/nonexistent/mixlink.sock"));
    }
}
