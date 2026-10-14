//! Blocking client for the wavelinkd socket (used by the CLI).
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::protocol::{Command, ErrorBody, ErrorCode, Event, Request, ServerMessage, to_line};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("cannot reach wavelinkd at {path}: {source}")]
    Connect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("connection error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid message from wavelinkd: {0}")]
    Protocol(String),
    #[error("{0}")]
    Daemon(ErrorBody),
}

pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: u64,
}

impl Client {
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).map_err(|source| ClientError::Connect {
            path: path.to_path_buf(),
            source,
        })?;
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
        })
    }

    /// Sends a command and waits for its response, skipping events in between.
    pub fn call(&mut self, command: Command) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        self.writer
            .write_all(to_line(&Request { id, command }).as_bytes())?;
        loop {
            if let ServerMessage::Response(response) = self.read_message()? {
                if response.id != id {
                    continue;
                }
                return if response.ok {
                    Ok(response.result.unwrap_or(Value::Null))
                } else {
                    Err(ClientError::Daemon(response.error.unwrap_or_else(|| {
                        ErrorBody::new(ErrorCode::Internal, "error without details")
                    })))
                };
            }
        }
    }

    /// Blocks until the next event arrives (after a `subscribe` call).
    pub fn next_event(&mut self) -> Result<Event, ClientError> {
        loop {
            if let ServerMessage::Event(event) = self.read_message()? {
                return Ok(event);
            }
        }
    }

    fn read_message(&mut self) -> Result<ServerMessage, ClientError> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(ClientError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "wavelinkd closed the connection",
            )));
        }
        serde_json::from_str(&line).map_err(|e| ClientError::Protocol(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ErrorCode, Request};
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    /// Answers exactly one request: first an unrelated event, then `reply(id)`.
    fn fake_daemon(path: &Path, reply: fn(u64) -> String) -> std::thread::JoinHandle<()> {
        let listener = UnixListener::bind(path).unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let req: Request = serde_json::from_str(&line).unwrap();
            let mut w = stream;
            w.write_all(
                b"{\"event\":\"meters\",\"channels\":{},\"monitor\":[0,0],\"stream\":[0,0]}\n",
            )
            .unwrap();
            w.write_all(reply(req.id).as_bytes()).unwrap();
        })
    }

    #[test]
    fn call_skips_events_and_returns_result() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.sock");
        let server = fake_daemon(&path, |id| {
            format!("{{\"id\":{id},\"ok\":true,\"result\":{{\"x\":1}}}}\n")
        });
        let mut client = Client::connect(&path).unwrap();
        let value = client.call(Command::GetState).unwrap();
        assert_eq!(value["x"], 1);
        server.join().unwrap();
    }

    #[test]
    fn daemon_errors_become_client_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.sock");
        let server = fake_daemon(&path, |id| {
            format!(
                "{{\"id\":{id},\"ok\":false,\"error\":{{\"code\":\"not_found\",\"message\":\"nope\",\"options\":[\"Music\"]}}}}\n"
            )
        });
        let mut client = Client::connect(&path).unwrap();
        match client.call(Command::GetState) {
            Err(ClientError::Daemon(body)) => {
                assert_eq!(body.code, ErrorCode::NotFound);
                assert_eq!(body.options, vec!["Music"]);
            }
            other => panic!("unexpected {other:?}"),
        }
        server.join().unwrap();
    }

    #[test]
    fn connect_error_names_the_socket() {
        let err = Client::connect(Path::new("/nonexistent/wl.sock"))
            .err()
            .unwrap();
        assert!(err.to_string().contains("/nonexistent/wl.sock"));
    }
}
