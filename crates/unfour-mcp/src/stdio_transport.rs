//! One bounded business-call slot; protocol control messages never wait for it.
use crate::{
    call_control::{with_cancellation, CALL_TIMEOUT},
    protocol, McpServer,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufRead, Write},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tokio::sync::watch;

enum Event {
    Line(io::Result<String>),
    Eof,
    Completed(Option<Value>),
}
struct Active {
    id: Value,
    cancel: watch::Sender<bool>,
    started: Instant,
    suppressed: bool,
}
impl Drop for Active {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

pub(crate) fn run<R: BufRead + Send + 'static, W: Write>(
    server: Arc<McpServer>,
    reader: R,
    writer: &mut W,
    idle: Option<Duration>,
) -> io::Result<()> {
    // Bound queued protocol messages as well as execution threads.
    let (tx, rx) = mpsc::sync_channel(64);
    let input = tx.clone();
    std::thread::spawn(move || {
        for line in reader.lines() {
            if input.send(Event::Line(line)).is_err() {
                return;
            }
        }
        let _ = input.send(Event::Eof);
    });
    let mut active: Option<Active> = None;
    let mut queued: VecDeque<Value> = VecDeque::new();
    let mut eof: Option<Instant> = None;
    let mut last_message = Instant::now();
    loop {
        if eof.is_some_and(|at| at.elapsed() >= Duration::from_secs(2)) {
            return Ok(());
        }
        if active.is_none() {
            if let Some(message) = queued.pop_front() {
                active = Some(start_call(server.clone(), tx.clone(), message));
            } else if eof.is_some() {
                return Ok(());
            }
        }
        if let Some(call) = active.as_mut() {
            if !call.suppressed && call.started.elapsed() >= CALL_TIMEOUT {
                let _ = call.cancel.send(true);
                call.suppressed = true;
                if !write(writer,protocol::error(call.id.clone(),-32000,"MCP_CALL_TIMEOUT: execution may have partially completed; inspect state before retrying"))? { return Ok(()); }
                // Do not release the slot until its worker actually exits.
            }
        } else if idle.is_some_and(|timeout| last_message.elapsed() >= timeout) {
            return Ok(());
        }
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Event::Eof) => {
                eof = Some(Instant::now());
            }
            Ok(Event::Completed(response)) => {
                if let Some(call) = active.take() {
                    if !call.suppressed {
                        if let Some(response) = response {
                            if !write(writer, response)? {
                                return Ok(());
                            }
                        }
                    }
                }
                last_message = Instant::now();
            }
            Ok(Event::Line(line)) => {
                let line = line?;
                last_message = Instant::now();
                if line.trim().is_empty() {
                    continue;
                }
                let message = match serde_json::from_str::<Value>(&line) {
                    Ok(value) => value,
                    Err(_) => {
                        if !write(writer, protocol::error(Value::Null, -32700, "Parse error"))? {
                            return Ok(());
                        }
                        continue;
                    }
                };
                let valid = message["jsonrpc"] == "2.0";
                let method = message["method"].as_str();
                if valid && method == Some("notifications/cancelled") && message.get("id").is_none()
                {
                    queued.retain(|m| m.get("id") != message["params"].get("requestId"));
                    if let Some(call) = active.as_mut() {
                        if message["params"].get("requestId") == Some(&call.id) {
                            call.suppressed = true;
                            let _ = call.cancel.send(true);
                        }
                    }
                    continue;
                }
                if valid && method == Some("tools/call") {
                    // Requests need an ID; notifications must never execute tools.
                    let Some(id) = message
                        .get("id")
                        .filter(|id| id.is_string() || id.as_i64().is_some())
                        .cloned()
                    else {
                        continue;
                    };
                    if active.as_ref().is_some_and(|call| call.id == id)
                        || queued.iter().any(|m| m["id"] == id)
                    {
                        if !write(
                            writer,
                            protocol::error(id, -32600, "Duplicate in-flight request ID"),
                        )? {
                            return Ok(());
                        }
                        continue;
                    }
                    if queued.len() >= 16 {
                        if !write(
                            writer,
                            protocol::error(id, -32000, "MCP_SERVER_BUSY: tool call queue is full"),
                        )? {
                            return Ok(());
                        }
                        continue;
                    }
                    queued.push_back(message);
                } else if let Some(response) = server.handle_message(&message) {
                    if !write(writer, response)? {
                        return Ok(());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn start_call(server: Arc<McpServer>, tx: mpsc::SyncSender<Event>, message: Value) -> Active {
    let (cancel, receiver) = watch::channel(false);
    let active = Active {
        id: message["id"].clone(),
        cancel,
        started: Instant::now(),
        suppressed: false,
    };
    std::thread::spawn(move || {
        let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_cancellation(receiver, || server.handle_message(&message))
        }))
        .unwrap_or_else(|_| {
            Some(protocol::error(
                message["id"].clone(),
                -32603,
                "Internal error",
            ))
        });
        let _ = tx.send(Event::Completed(response));
    });
    active
}

fn write<W: Write>(writer: &mut W, value: Value) -> io::Result<bool> {
    match writeln!(writer, "{value}").and_then(|_| writer.flush()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(e) => Err(e),
    }
}
