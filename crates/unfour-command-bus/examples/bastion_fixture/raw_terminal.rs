//! Raw terminal protocol simulator. Fixed synthetic replies only, no OS exec.
use base64::{engine::general_purpose::STANDARD, Engine};
use russh::{server::Session, ChannelId};

#[derive(Clone, Default)]
pub struct RawTerminal {
    line: Vec<u8>,
    session: Option<String>,
    nonce: Option<String>,
    payload: Vec<u8>,
    startup: bool,
    curl: bool,
    probe: bool,
    input_lines: usize,
    recovery_faults: usize,
    click_sample: bool,
}

impl RawTerminal {
    pub fn active(&self) -> bool {
        self.startup || self.session.is_some() || !self.line.is_empty()
    }

    pub fn feed(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), russh::Error> {
        for &byte in data {
            if byte == 3 {
                self.restore(channel, session)?;
            } else if matches!(byte, b'\r' | b'\n') {
                let line = std::mem::take(&mut self.line);
                if !line.is_empty() {
                    self.line(channel, &line, session)?;
                }
            } else {
                assert!(self.line.len() < 4096);
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
        if line.starts_with(b" sh -c '") {
            self.curl = line.windows(14).any(|s| s == b"UNFOUR_CURL_V4");
            self.startup = true;
            if self.curl {
                let sid = std::str::from_utf8(line)
                    .unwrap()
                    .split_whitespace()
                    .last()
                    .unwrap();
                self.session = Some(sid.into());
                return frame(channel, &format!("{sid}:LOAD"), session);
            }
            assert!(line.windows(13).any(|p| p == b"UNFOUR_RAW_V3"));
            session.data(channel, [line, b"\r\n"].concat())?;
            self.startup = true;
            frame(channel, "UNFOUR_RAW_V3", session)?;
            println!("FIXTURE_RAW_BOOTSTRAP");
        } else if self.startup {
            if self.curl {
                if line == b"+" {
                    return frame(
                        channel,
                        &format!("{}:PULSE", self.session.as_ref().unwrap()),
                        session,
                    );
                }
                let script = std::str::from_utf8(line).unwrap();
                let sid = self.session.clone().unwrap();
                self.startup = false;
                self.probe = !script.contains("UNFOUR_CURL_RESULT_");
                self.session = Some(sid.clone());
                self.nonce = Some(sid.clone());
                // Advertise the confirmed lane. Real PTY QA covers digest negotiation.
                return frame(channel, &format!("{sid}:READY:4:b64:-:D"), session);
            }
            self.session = Some(String::from_utf8(line.to_vec()).unwrap());
            self.startup = false;
            if std::env::var_os("UNFOUR_BASTION_QA_RAW_RECOVERY").is_some()
                && self.recovery_faults == 0
            {
                self.recovery_faults += 1;
                frame(
                    channel,
                    &format!("{}:BASE64_MISSING", self.session.as_ref().unwrap()),
                    session,
                )?;
                println!("FIXTURE_RAW_TOOL_FAILURE");
                // The child restores on its own before any input is accepted.
                return self.restore(channel, session);
            }
            frame(
                channel,
                &format!("{}:READY:3:240:b64", self.session.as_ref().unwrap()),
                session,
            )?;
        } else if line == b"Q"
            || (self.curl && line == format!("Q:{}", self.session.as_ref().unwrap()).as_bytes())
        {
            if self.click_sample {
                // Q arrives only after the native client decoded the complete result.
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap();
                println!("FIXTURE_COMPLETE_OBSERVED {}", now.as_millis());
            }
            self.restore(channel, session)?;
        } else if self.nonce.is_none() {
            self.nonce = Some(String::from_utf8(line.to_vec()).unwrap());
            frame(
                channel,
                &format!("{}:ACCEPT", self.nonce.as_ref().unwrap()),
                session,
            )?;
        } else if line.starts_with(b"E:") {
            assert_eq!(
                line,
                format!("E:{}:{}", self.input_lines, self.payload.len()).as_bytes()
            );
            frame(
                channel,
                &format!(
                    "{}:INPUT_END:{}:{}",
                    self.nonce.as_ref().unwrap(),
                    self.input_lines,
                    self.payload.len()
                ),
                session,
            )?;
            let command =
                String::from_utf8(STANDARD.decode(std::mem::take(&mut self.payload)).unwrap())
                    .unwrap();
            let nonce = self.nonce.as_ref().unwrap();
            frame(channel, &format!("{nonce}:BEGIN"), session)?;
            if std::env::var_os("UNFOUR_BASTION_QA_RAW_RECOVERY").is_some()
                && self.recovery_faults == 1
            {
                self.recovery_faults += 1;
                assert!(command.contains("UNFOUR_TRANSPORT_ROUNDTRIP:"));
                println!("FIXTURE_RAW_END_WITHHELD");
                // No HTTP request: hold only the synthetic capability check.
                return Ok(());
            }
            let response = if self.probe {
                println!("FIXTURE_CURL_PROBE");
                Some((command.as_bytes().to_vec(), 0))
            } else {
                self.click_sample = command.contains("http://terminal.fixture.invalid/small");
                synthetic(&command, nonce)
            };
            let Some((mut stdout, code)) = response else {
                println!("FIXTURE_CURL_WAIT");
                return Ok(());
            };
            stdout.extend_from_slice(format!("\x1e{nonce}:STATUS:{code}\x1f").as_bytes());
            for chunk in STANDARD.encode(stdout).as_bytes().chunks(4096) {
                session.data(channel, [chunk, b"\r\n"].concat())?;
            }
            frame(channel, &format!("{nonce}:END:0"), session)?;
        } else if let Some(encoded) = line.strip_prefix(b"D") {
            assert!(encoded.len() <= 240);
            self.payload.extend_from_slice(encoded);
            assert!(self.payload.len() <= (4 * 1024 * 1024usize).div_ceil(3) * 4);
            self.input_lines += 1;
            frame(
                channel,
                &format!(
                    "{}:INPUT:{}:{}:{}",
                    self.nonce.as_ref().unwrap(),
                    self.input_lines,
                    encoded.len(),
                    std::str::from_utf8(encoded).unwrap()
                ),
                session,
            )?;
        } else {
            panic!("invalid synthetic raw input");
        }
        Ok(())
    }

    fn restore(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), russh::Error> {
        if let Some(nonce) = self.session.take() {
            frame(channel, &format!("{nonce}:RESTORED"), session)?;
        }
        let recovery_faults = self.recovery_faults;
        *self = Self::default();
        self.recovery_faults = recovery_faults;
        println!("FIXTURE_RAW_RESTORED");
        Ok(())
    }
}

fn synthetic(command: &str, nonce: &str) -> Option<(Vec<u8>, u8)> {
    let transport_text = format!(
        "UNFOUR_TRANSPORT_ROUNDTRIP:{}",
        "0123456789abcdef".repeat(60)
    );
    if command.contains(&transport_text) && !command.contains("terminal.fixture.invalid") {
        assert!(!command.contains("curl") && !command.contains("http"));
        assert!(command.ends_with("exit 23"));
        let mut output = transport_text.into_bytes();
        output.extend_from_slice(b"\n\0\x01\x1e\x1f\x7f\xff");
        println!("FIXTURE_TRANSPORT_PROBE");
        return Some((output, 23));
    }
    if command.contains("UNFOUR_CURL_TOOLS") && !command.contains("terminal.fixture.invalid") {
        println!("FIXTURE_CURL_PROBE");
        return Some((b"UNFOUR_CURL_TOOLS".to_vec(), 0));
    }
    assert!(command.contains("http://terminal.fixture.invalid/"));
    let nonce = if let Some((_, metadata)) = command.split_once("UNFOUR_CURL_RESULT_") {
        // Historical V3 fixture input contains the wrapper and its separate HTTP nonce.
        assert!(command.contains("--max-filesize 10485760") && command.contains("--max-time "));
        &metadata[..32]
    } else {
        assert!(command.contains("max-filesize = 10485760"));
        nonce
    };
    assert!(command.contains("request = \"POST\""));
    assert!(command.contains("content-type: application/json"));
    assert!(command.contains("authorization: Bearer "));
    assert!(
        command.contains("UNFOUR_CURL_QA_TOKEN_9f4c9e40")
            || command.contains("synthetic-api-secret")
    );
    assert!(command.contains("data-binary = \"{\\\"input\\\":"));
    assert!(command.contains("中文") || command.contains("北京"));
    if command.contains("/timeout") {
        assert!(
            command.contains("max-time = 30\n") || command.contains("--max-time 30 --config -")
        );
        return None;
    }
    if command.contains("/wait") {
        return None;
    }
    if command.contains("/connect-failed") {
        println!("FIXTURE_CURL_CONNECT_FAILED");
        return Some((Vec::new(), 7));
    }
    let status = if command.contains("/error") {
        500
    } else if command.contains("/redirect") {
        302
    } else {
        200
    };
    let body = if command.contains("http://terminal.fixture.invalid/small") {
        std::thread::sleep(std::time::Duration::from_millis(200));
        format!("{{\"value\":\"{}\"}}", "x".repeat(85))
    } else {
        serde_json::json!({"ok": status == 200, "input":"中文引号'\"换行\n", "values": vec![0.125; 17000]}).to_string()
    };
    assert!(nonce.bytes().all(|b| b.is_ascii_hexdigit()));
    let stdout = format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Repeat: one\r\nX-Repeat: two\r\n\r\n{body}\nUNFOUR_CURL_RESULT_{nonce}:{status}:{}:0.120:0.195\n", body.len(), body.len());
    println!("FIXTURE_CURL_RESPONSE {status}");
    Some((stdout.into_bytes(), 0))
}

fn frame(channel: ChannelId, value: &str, session: &mut Session) -> Result<(), russh::Error> {
    session.data(channel, vec![0x1e])?;
    session.data(channel, value.as_bytes().to_vec())?;
    session.data(channel, vec![0x1f])
}
