//! Deterministic SSH/PTTY protocol peer; never executes arbitrary OS commands.
use super::*;

#[derive(Default)]
pub(super) struct Terminal {
    line: Vec<u8>,
    bootstrapped: bool,
    session: Option<String>,
    step: Option<String>,
    payload: Vec<u8>,
    raw: bool,
    input_lines: usize,
    input_encoded: usize,
}
impl Terminal {
    pub(super) fn feed(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut Session,
        state: &Arc<Mutex<State>>,
    ) -> Result<(), russh::Error> {
        {
            let mut state = state.lock().unwrap();
            assert!(state.terminal_wire.len() + data.len() < 512 * 1024);
            state.terminal_wire.extend_from_slice(data);
        }
        if self.session.is_none()
            && !self.bootstrapped
            && self.line.is_empty()
            && data.first() != Some(&b' ')
            && !data.contains(&3)
        {
            session.data(id, data.to_vec())?;
            return Ok(());
        }
        for &byte in data {
            if byte == 3 {
                if !state.lock().unwrap().terminal_ignore_interrupt {
                    self.restore(id, session)?;
                }
                continue;
            }
            if byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                self.line(id, &line, session, state)?;
            } else {
                self.line.push(byte);
                assert!(self.line.len() < 4096);
            }
        }
        Ok(())
    }
    fn line(
        &mut self,
        id: ChannelId,
        line: &[u8],
        session: &mut Session,
        state: &Arc<Mutex<State>>,
    ) -> Result<(), russh::Error> {
        // Echo all inputs, including payload, to verify that echo cannot act as
        // a control frame or leak into ordinary terminal output.
        session.data(id, [line, b"\r\n"].concat())?;
        if line.starts_with(b" sh -c '") {
            if state.lock().unwrap().terminal_login_prompt {
                session.data(id, b"\x1b]0;private-login-title\x07lo".to_vec())?;
                session.data(id, b"gin: ".to_vec())?;
            } else {
                self.bootstrapped = true;
                self.raw = line
                    .windows(b"UNFOUR_RAW_V3".len())
                    .any(|part| part == b"UNFOUR_RAW_V3");
                if (self.raw && state.lock().unwrap().terminal_raw_stall == Some("STARTUP"))
                    || (!self.raw && state.lock().unwrap().terminal_task_stall == Some("STARTUP"))
                {
                    return Ok(());
                }
                frame(
                    id,
                    if self.raw {
                        "UNFOUR_RAW_V3"
                    } else {
                        "UNFOUR_TASK_V2"
                    },
                    session,
                )?;
            }
        } else if self.bootstrapped {
            let nonce = String::from_utf8(line.to_vec()).unwrap();
            self.bootstrapped = false;
            self.session = Some(nonce.clone());
            if !self.raw && state.lock().unwrap().terminal_task_stall == Some("READY") {
                return Ok(());
            }
            if self.raw {
                let (missing, printf, stall, foreign) = {
                    let state = state.lock().unwrap();
                    (
                        state.terminal_raw_missing_tool,
                        state.terminal_raw_printf_external,
                        state.terminal_raw_stall,
                        state.terminal_raw_foreign_restore,
                    )
                };
                if missing || printf {
                    let error = if missing {
                        "BASE64_MISSING"
                    } else {
                        "PRINTF_UNSUPPORTED"
                    };
                    frame(id, &format!("{nonce}:{error}"), session)?;
                    if foreign {
                        frame(id, "foreign-session:RESTORED", session)?;
                        return Ok(());
                    }
                    return self.restore(id, session);
                }
                if stall == Some("READY") {
                    return Ok(());
                }
            }
            frame(
                id,
                &if self.raw {
                    format!(
                        "{nonce}:READY:3:240:{}",
                        if state.lock().unwrap().terminal_octal {
                            "octal"
                        } else {
                            "b64"
                        }
                    )
                } else {
                    format!("{nonce}:READY")
                },
                session,
            )?;
        } else if self.session.is_some() {
            if line == b"Q" {
                if !state.lock().unwrap().terminal_ignore_restore {
                    self.restore_with_gate(id, session, state)?;
                }
            } else if let Some(nonce) = self.step.clone() {
                if line.starts_with(b"E:") {
                    if self.raw {
                        let (stall, corrupt) = {
                            let mut state = state.lock().unwrap();
                            state.terminal_raw_input_end_received += 1;
                            state.terminal_raw_pipelined_input |=
                                state.terminal_raw_pending_input_acks > 0;
                            (
                                state.terminal_raw_stall,
                                state.terminal_raw_corrupt_end_total,
                            )
                        };
                        // Model a relay dropping the completion line entirely.
                        // The peer never starts the command or emits BEGIN.
                        if stall == Some("INPUT_END") {
                            return Ok(());
                        }
                        let expected = format!("E:{}:{}", self.input_lines, self.input_encoded);
                        if corrupt || line != expected.as_bytes() {
                            frame(id, &format!("{nonce}:INPUT_INVALID"), session)?;
                            return self.restore(id, session);
                        }
                        if stall != Some("INPUT_END_ACK") {
                            let wrong = state.lock().unwrap().terminal_raw_corrupt_end_ack;
                            frame(
                                id,
                                &format!(
                                    "{nonce}:INPUT_END:{}:{}",
                                    self.input_lines,
                                    self.input_encoded + usize::from(wrong)
                                ),
                                session,
                            )?;
                        }
                    } else {
                        let total = format!("{}:{}", self.input_lines, self.input_encoded);
                        assert_eq!(line, format!("E:{total}").as_bytes());
                        frame(id, &format!("{nonce}:INPUT_END:{total}"), session)?;
                    }
                    let mut payload = std::mem::take(&mut self.payload);
                    if self.raw && !state.lock().unwrap().terminal_octal {
                        use base64::{engine::general_purpose::STANDARD, Engine};
                        payload = STANDARD.decode(payload).unwrap();
                    }
                    let command = String::from_utf8(payload).unwrap();
                    state
                        .lock()
                        .unwrap()
                        .terminal_commands
                        .push(command.clone());
                    if self.raw && state.lock().unwrap().terminal_raw_stall == Some("BEGIN") {
                        return Ok(());
                    }
                    frame(id, &format!("{nonce}:BEGIN"), session)?;
                    if self.raw {
                        use base64::{engine::general_purpose::STANDARD, Engine};
                        if command == "raw-hold"
                            || state.lock().unwrap().terminal_raw_stall == Some("END")
                        {
                            return Ok(());
                        }
                        let (mut output, code) = if command.contains("UNFOUR_TRANSPORT_ROUNDTRIP:")
                        {
                            let text = command
                                .strip_prefix("command printf '%s' '")
                                .unwrap()
                                .split_once("'; command printf ")
                                .unwrap()
                                .0;
                            let mut output = text.as_bytes().to_vec();
                            output.extend_from_slice(b"\n\0\x01\x1e\x1f\x7f\xff");
                            let state = state.lock().unwrap();
                            if state.terminal_transport_corrupt_output {
                                output[0] ^= 1;
                            }
                            (output, state.terminal_transport_code.unwrap_or(23))
                        } else if command.contains("UNFOUR_CURL_TOOLS") {
                            (
                                b"UNFOUR_CURL_TOOLS".to_vec(),
                                state.lock().unwrap().terminal_curl_code.unwrap_or(0),
                            )
                        } else if state.lock().unwrap().terminal_line_delay.is_some() {
                            (vec![b'x'; 97], 0)
                        } else if command == "raw-large" {
                            ((0..=255).cycle().take(150_000).collect(), 23)
                        } else if command == "raw-huge" {
                            (vec![b'x'; 10 * 1024 * 1024], 0)
                        } else {
                            (command.as_bytes().to_vec(), 0)
                        };
                        output
                            .extend_from_slice(format!("\x1e{nonce}:STATUS:{code}\x1f").as_bytes());
                        let encoded = STANDARD.encode(output);
                        for line in encoded.as_bytes().chunks(4096) {
                            session.data(id, [line, b"\r\n"].concat())?;
                        }
                        if command != "raw-status-only" {
                            if state.lock().unwrap().terminal_raw_auto_restore {
                                let parent = self.session.take().unwrap();
                                session.data(
                                    id,
                                    format!("\x1e{nonce}:END:0\x1f\x1e{parent}:RESTORED\x1f")
                                        .into_bytes(),
                                )?;
                            } else {
                                frame(id, &format!("{nonce}:END:0"), session)?;
                            }
                        }
                        self.step = None;
                        return Ok(());
                    }
                    if command == "hold" {
                        session.data(id, b"waiting\n".to_vec())?;
                        return Ok(());
                    }
                    if command == "status-only" {
                        frame(id, &format!("{nonce}:STATUS:0"), session)?;
                        return Ok(());
                    }
                    let code = if command == "fail" || command.ends_with("exit 23") {
                        23
                    } else {
                        0
                    };
                    if command.contains("UNFOUR_TERMINAL_PROBE") {
                        session.data(id, b"UNFOUR_TERMINAL_PROBE\r\n".to_vec())?;
                    } else {
                        let output = format!("output:{command}\n").into_bytes();
                        for chunk in output.chunks(3) {
                            session.data(id, chunk.to_vec())?;
                        }
                    }
                    frame(id, &format!("{nonce}:STATUS:{code}"), session)?;
                    frame(id, &format!("{nonce}:END"), session)?;
                    self.step = None;
                } else {
                    assert_eq!(line.first(), Some(&b'D'));
                    assert!(line.len() <= 241);
                    if !self.raw
                        && state.lock().unwrap().terminal_task_drop_line
                            == Some(self.input_lines + 1)
                    {
                        return Ok(());
                    }
                    self.input_lines += 1;
                    let mut received = line[1..].to_vec();
                    if self.raw {
                        let mut state = state.lock().unwrap();
                        state.terminal_raw_input_lines.push(line.len() + 1);
                        state.terminal_raw_pipelined_input |=
                            state.terminal_raw_pending_input_acks > 0;
                        if state.terminal_raw_truncate_input_line == Some(self.input_lines) {
                            // Drop one encoded group without making the line empty.
                            received.truncate(received.len().saturating_sub(4));
                        }
                    }
                    if state.lock().unwrap().terminal_raw_mutate_input_line
                        == Some(self.input_lines)
                    {
                        let last = received.last_mut().unwrap();
                        *last = if *last == b'0' { b'1' } else { b'0' };
                    }
                    self.input_encoded += received.len();
                    if self.raw && !state.lock().unwrap().terminal_octal {
                        // Keep the bytes as received, including injected faults.
                        // The peer decodes only after the client commits with E.
                        self.payload.extend_from_slice(&received);
                    } else {
                        for escape in received.chunks_exact(5) {
                            assert_eq!(&escape[..2], b"\\0");
                            self.payload.push(
                                u8::from_str_radix(std::str::from_utf8(&escape[2..]).unwrap(), 8)
                                    .unwrap(),
                            );
                        }
                    }
                    if !self.raw || state.lock().unwrap().terminal_raw_stall != Some("INPUT_ACK") {
                        let index = self.input_lines
                            + usize::from(state.lock().unwrap().terminal_raw_wrong_input_index);
                        input_ack(
                            id,
                            format!(
                                "{nonce}:INPUT:{index}:{}:{}",
                                received.len(),
                                std::str::from_utf8(&received).unwrap()
                            ),
                            session,
                            state,
                        )?;
                    }
                }
            } else {
                let nonce = String::from_utf8(line.to_vec()).unwrap();
                if self.raw && state.lock().unwrap().terminal_raw_early_restore {
                    let parent = self.session.take().unwrap();
                    return session.data(
                        id,
                        format!("\x1e{nonce}:ACCEPT\x1f\x1e{parent}:RESTORED\x1f").into_bytes(),
                    );
                }
                if self.raw && state.lock().unwrap().terminal_raw_stall == Some("ACCEPT") {
                    return Ok(());
                }
                frame(id, &format!("{nonce}:ACCEPT"), session)?;
                self.step = Some(nonce);
                self.input_lines = 0;
                self.input_encoded = 0;
            }
        }
        Ok(())
    }
    fn restore(&mut self, id: ChannelId, session: &mut Session) -> Result<(), russh::Error> {
        if let Some(nonce) = self.session.take() {
            frame(id, &format!("{nonce}:RESTORED"), session)?;
        }
        self.bootstrapped = false;
        self.step = None;
        self.payload.clear();
        self.line.clear();
        Ok(())
    }

    fn restore_with_gate(
        &mut self,
        id: ChannelId,
        session: &mut Session,
        state: &Arc<Mutex<State>>,
    ) -> Result<(), russh::Error> {
        let gate = state.lock().unwrap().terminal_raw_restore_gate.clone();
        if self.raw {
            if let (Some(gate), Some(nonce)) = (gate, self.session.clone()) {
                self.session = None;
                self.bootstrapped = false;
                self.step = None;
                self.payload.clear();
                self.line.clear();
                state.lock().unwrap().terminal_raw_restores_waiting += 1;
                let handle = session.handle();
                let state = state.clone();
                tokio::spawn(async move {
                    let Ok(permit) = gate.acquire().await else {
                        return;
                    };
                    permit.forget();
                    state.lock().unwrap().terminal_raw_restores_waiting -= 1;
                    if handle
                        .data(id, format!("\x1e{nonce}:RESTORED\x1f").into_bytes())
                        .await
                        .is_ok()
                    {
                        state.lock().unwrap().terminal_raw_restores_sent += 1;
                    }
                });
                return Ok(());
            }
        }
        self.restore(id, session)
    }
}
fn input_ack(
    id: ChannelId,
    value: String,
    session: &mut Session,
    state: &Arc<Mutex<State>>,
) -> Result<(), russh::Error> {
    let gate = state.lock().unwrap().terminal_raw_ack_gate.clone();
    if let Some(gate) = gate {
        state.lock().unwrap().terminal_raw_pending_input_acks += 1;
        let handle = session.handle();
        let state = state.clone();
        // Keep reading client data while this ACK is withheld. Blocking the
        // handler itself would hide a driver that pipelines without waiting.
        tokio::spawn(async move {
            let Ok(permit) = gate.acquire().await else {
                return;
            };
            permit.forget();
            state.lock().unwrap().terminal_raw_pending_input_acks -= 1;
            let sent = handle
                .data(id, format!("\x1e{value}\x1f").into_bytes())
                .await;
            let mut state = state.lock().unwrap();
            if sent.is_ok() {
                state.terminal_raw_input_acks_sent += 1;
            }
        });
        Ok(())
    } else {
        frame(id, &value, session)?;
        state.lock().unwrap().terminal_raw_input_acks_sent += 1;
        Ok(())
    }
}
fn frame(id: ChannelId, frame: &str, session: &mut Session) -> Result<(), russh::Error> {
    session.data(id, vec![0x1e])?;
    for chunk in frame.as_bytes().chunks(2) {
        session.data(id, chunk.to_vec())?;
    }
    session.data(id, vec![0x1f])
}
