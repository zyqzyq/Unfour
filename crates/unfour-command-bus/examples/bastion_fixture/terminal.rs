//! Bounded protocol simulator: only fixed synthetic commands, never OS exec.
use russh::{server::Session, ChannelId};

#[derive(Default)]
pub struct Terminal {
    line: Vec<u8>,
    session: Option<String>,
    step: Option<String>,
    payload: Vec<u8>,
    bootstrap: bool,
    input_lines: usize,
}

impl Terminal {
    pub fn active(&self) -> bool {
        self.bootstrap || self.session.is_some() || !self.line.is_empty()
    }

    pub fn feed(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), russh::Error> {
        for &byte in data {
            if byte == 3 && self.active() {
                self.restore(channel, session)?;
                continue;
            }
            if matches!(byte, b'\r' | b'\n') {
                let line = std::mem::take(&mut self.line);
                if !line.is_empty() {
                    self.line(channel, &line, session)?;
                }
            } else if self.line.len() < 4096 {
                self.line.push(byte);
            }
        }
        Ok(())
    }

    fn line(
        &mut self,
        channel: ChannelId,
        line: &[u8],
        session: &mut Session,
    ) -> Result<(), russh::Error> {
        if line.starts_with(b" sh -c '") && line.windows(14).any(|s| s == b"UNFOUR_TASK_V2") {
            // Deliberately echo the bootstrap: its escaped text must not count
            // as a handshake or appear in the task/terminal display.
            session.data(channel, [line, b"\r\n"].concat())?;
            self.bootstrap = true;
            frame(channel, "UNFOUR_TASK_V2", session)?;
            println!("FIXTURE_TERMINAL_BOOTSTRAP");
        } else if self.bootstrap {
            let nonce = std::str::from_utf8(line).unwrap().to_owned();
            self.bootstrap = false;
            frame(channel, &format!("{nonce}:READY"), session)?;
            self.session = Some(nonce);
        } else if self.session.is_some() {
            if line == b"Q" {
                self.restore(channel, session)?;
            } else if let Some(step) = &self.step {
                if line.starts_with(b"E:") {
                    let total = format!("{}:{}", self.input_lines, self.payload.len() * 5);
                    assert_eq!(line, format!("E:{total}").as_bytes());
                    frame(channel, &format!("{step}:INPUT_END:{total}"), session)?;
                    let step = step.clone();
                    let command = String::from_utf8(std::mem::take(&mut self.payload)).unwrap();
                    frame(channel, &format!("{step}:BEGIN"), session)?;
                    let code = if command == "fixture-terminal-ok" {
                        session.data(
                            channel,
                            b"UNFOUR_TERMINAL_STDOUT\r\nUNFOUR_TERMINAL_STDERR\r\n".to_vec(),
                        )?;
                        println!("FIXTURE_TERMINAL_TASK_OK");
                        0
                    } else if command == "fixture-terminal-fail" {
                        println!("FIXTURE_TERMINAL_TASK_FAIL");
                        23
                    } else if command == "fixture-terminal-wait" {
                        session.data(channel, b"UNFOUR_TERMINAL_WAITING\r\n".to_vec())?;
                        println!("FIXTURE_TERMINAL_TASK_WAIT");
                        return Ok(());
                    } else if command.contains("UNFOUR") && command.contains("printf") {
                        // The fixed terminal capability commands are intentionally
                        // handled without exposing an arbitrary command executor.
                        let code = if command.contains("23") { 23 } else { 0 };
                        session.data(channel, b"UNFOUR_TERMINAL_PROBE\r\n".to_vec())?;
                        println!("FIXTURE_TERMINAL_PROBE {code}");
                        code
                    } else {
                        panic!("unsupported synthetic terminal command");
                    };
                    frame(channel, &format!("{step}:STATUS:{code}"), session)?;
                    frame(channel, &format!("{step}:END"), session)?;
                    self.step = None;
                } else if line.starts_with(b"D") {
                    assert!(line.len() <= 241);
                    self.input_lines += 1;
                    let part = std::str::from_utf8(&line[1..]).unwrap();
                    frame(
                        channel,
                        &format!("{step}:INPUT:{}:{}:{part}", self.input_lines, part.len()),
                        session,
                    )?;
                    for escape in line[1..].chunks_exact(5) {
                        assert_eq!(&escape[..2], b"\\0");
                        self.payload.push(
                            u8::from_str_radix(std::str::from_utf8(&escape[2..]).unwrap(), 8)
                                .unwrap(),
                        );
                    }
                    assert!(self.payload.len() <= 64 * 1024);
                } else {
                    panic!("invalid terminal payload frame");
                }
            } else {
                let step = std::str::from_utf8(line).unwrap().to_owned();
                frame(channel, &format!("{step}:ACCEPT"), session)?;
                self.step = Some(step);
                self.input_lines = 0;
            }
        }
        Ok(())
    }

    fn restore(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), russh::Error> {
        if let Some(nonce) = self.session.take() {
            frame(channel, &format!("{nonce}:RESTORED"), session)?;
        }
        self.step = None;
        self.payload.clear();
        self.line.clear();
        self.bootstrap = false;
        println!("FIXTURE_TERMINAL_RESTORED");
        Ok(())
    }
}

fn frame(channel: ChannelId, value: &str, session: &mut Session) -> Result<(), russh::Error> {
    // Split control frames to exercise the actual client's stream parser.
    session.data(channel, vec![0x1e])?;
    session.data(channel, value.as_bytes().to_vec())?;
    session.data(channel, vec![0x1f])
}
