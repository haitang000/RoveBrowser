//! Opt-in local transport. Network I/O never runs on the WebView2/UI thread.
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::mpsc::{self, SyncSender},
    time::{Duration, Instant},
};

const MAX_REQUEST: usize = 16 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    List {},
    Open { url: String },
    Navigate { id: String, url: String },
    Close { id: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    token: String,
    command: Command,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { result: serde_json::Value },
    Error { code: &'static str },
}

impl Response {
    pub fn ok(result: serde_json::Value) -> Self {
        Self::Ok { result }
    }

    pub fn error(code: &'static str) -> Self {
        Self::Error { code }
    }
}

#[derive(Debug)]
pub struct Request {
    pub command: Command,
    pub deadline: Instant,
    pub reply: SyncSender<Response>,
}

pub fn start(dispatch: impl Fn(Request) -> bool + Send + 'static) -> io::Result<()> {
    let (port, token) = match (
        std::env::var("ROVE_AGENT_PORT"),
        std::env::var("ROVE_AGENT_TOKEN"),
    ) {
        (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => return Ok(()),
        (Ok(port), Ok(token)) => (port, token),
        _ => {
            return Err(io::Error::other(
                "set both ROVE_AGENT_PORT and ROVE_AGENT_TOKEN",
            ))
        }
    };
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|p| *p >= 1024)
        .ok_or_else(|| io::Error::other("invalid ROVE_AGENT_PORT"))?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::other(
            "ROVE_AGENT_TOKEN must be 64 hex characters",
        ));
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    // Limit host dispatch to one command at a time. The listener lives until
    // the browser process exits.
    std::thread::Builder::new()
        .name("rove-agent".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                if stream.set_write_timeout(Some(IO_TIMEOUT)).is_err() {
                    continue;
                }
                let response = match receive(&mut stream, &token) {
                    Ok(command) => {
                        let (reply, receiver) = mpsc::sync_channel(1);
                        let deadline = Instant::now() + COMMAND_TIMEOUT;
                        if dispatch(Request {
                            command,
                            deadline,
                            reply,
                        }) {
                            receiver
                                .recv_timeout(COMMAND_TIMEOUT)
                                .unwrap_or_else(|err| {
                                    Response::error(match err {
                                        mpsc::RecvTimeoutError::Timeout => "timeout",
                                        mpsc::RecvTimeoutError::Disconnected => "unavailable",
                                    })
                                })
                        } else {
                            Response::error("unavailable")
                        }
                    }
                    Err(code) => Response::error(code),
                };
                if let Ok(mut bytes) = serde_json::to_vec(&response) {
                    bytes.push(b'\n');
                    let _ = stream.write_all(&bytes);
                }
            }
        })?;
    Ok(())
}

fn receive(stream: &mut TcpStream, token: &str) -> Result<Command, &'static str> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("invalid_request");
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| "invalid_request")?;
        let len = stream.read(&mut chunk).map_err(|_| "invalid_request")?;
        if len == 0 {
            return Err("invalid_request");
        }
        let newline = chunk[..len].iter().position(|&b| b == b'\n');
        let part = &chunk[..newline.unwrap_or(len)];
        if bytes.len() + part.len() > MAX_REQUEST {
            return Err("request_too_large");
        }
        bytes.extend_from_slice(part);
        if newline.is_some() {
            return decode(&bytes, token);
        }
    }
}

fn decode(bytes: &[u8], token: &str) -> Result<Command, &'static str> {
    let request: Envelope = serde_json::from_slice(bytes).map_err(|_| "invalid_request")?;
    let difference = request
        .token
        .bytes()
        .zip(token.bytes())
        .fold(0, |acc, (a, b)| acc | (a ^ b));
    if request.token.len() != token.len() || difference != 0 {
        return Err("unauthorized");
    }
    Ok(request.command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_authenticated_allowlisted_commands() {
        let list = br#"{"token":"secret","command":{"op":"list"}}"#;
        assert!(matches!(decode(list, "secret"), Ok(Command::List {})));
        assert_eq!(decode(list, "wrong!").unwrap_err(), "unauthorized");
        for invalid in [
            r#"{"command":{"op":"list"}}"#,
            r#"{"token":"secret","command":{"op":"eval","script":"1"}}"#,
            r#"{"token":"secret","command":{"op":"open","url":"https://example.com","focus":true}}"#,
            r#"{"token":"secret","command":{"op":"list","focus":true}}"#,
            r#"{"token":"secret","command":{"op":"close"}}"#,
            r#"{"token":"secret","command":{"op":"list"},"extra":true}"#,
        ] {
            assert_eq!(
                decode(invalid.as_bytes(), "secret").unwrap_err(),
                "invalid_request"
            );
        }
    }
}
