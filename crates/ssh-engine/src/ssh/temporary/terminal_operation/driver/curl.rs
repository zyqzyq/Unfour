use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use unfour_core::terminal_curl_contract::{
    CURL_SETUP, HTTP_VERSION_CHECK, RAW_BODY_CHECK, TRANSFER,
};

pub(in crate::ssh) const SAMPLE_BYTES: usize = 720;
// Four 240-character base64 lines with a B prefix and LF, followed by
// F:4:960:<64 hex digits> and LF. The probe sends exactly this many bytes.
const SAMPLE_WIRE_BYTES: usize = 4 * 242 + 73;

struct Burst {
    wire: Vec<u8>,
    totals: String,
}

impl Burst {
    fn bounded(config: &str) -> Option<Self> {
        if config.len() > SAMPLE_BYTES || config.is_empty() {
            return None;
        }
        let encoded = STANDARD.encode(config);
        let lines = encoded.as_bytes().chunks(240);
        let totals = format!("{}:{}", lines.len(), encoded.len());
        let digest = Sha256::digest(encoded.as_bytes());
        let mut wire = Vec::with_capacity(SAMPLE_WIRE_BYTES);
        for part in lines {
            wire.push(b'B');
            wire.extend_from_slice(part);
            wire.push(b'\n');
        }
        wire.extend_from_slice(format!("F:{totals}:{digest:x}\n").as_bytes());
        (wire.len() <= SAMPLE_WIRE_BYTES).then_some(Self { wire, totals })
    }
}

// No request config is accepted by the shell builder. Keep physical lines
// within the historical bootstrap limit even with single-quote expansion.
fn bootstrap(
    session: &str,
    raw: bool,
    probe: bool,
    candidate: bool,
    context: Option<&str>,
) -> String {
    let candidate = candidate && !raw;
    let select = if candidate && probe {
        r#"[ "$fmt" = b64 ] && [ "${#ctx}" = 64 ] && lane=B;"#.into()
    } else if let Some(context) = context.filter(|hash| candidate && valid_hash(hash)) {
        format!(r#"[ "$ctx" = {context} ] && lane=B;"#)
    } else {
        String::new()
    };
    // A cold request cannot use a cached capacity. Avoid remote process
    // launches just to compute an identity we would immediately discard.
    let identity = if select.is_empty() {
        ""
    } else {
        concat!(
            r#"if [ "$fmt" = b64 ] && [ -n "$PPID" ]; then "#,
            r#"key=$(tty && uname -n && printf %s "$PPID") && sum=$(printf %s "$key" | command sha256sum 2>/dev/null) && ctx=${sum%% *}; fi;"#
        )
    };
    let script = include_str!("../curl_receiver.sh")
        .lines()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("@LINE@", "\n")
        .replace("@SETUP@", CURL_SETUP)
        .replace("@CONTEXT@", identity)
        .replace(
            "@OPTIONS@",
            &format!(
                "{HTTP_VERSION_CHECK}{}",
                if raw { RAW_BODY_CHECK } else { "" }
            ),
        )
        .replace("@SELECT_LANE@", &select)
        .replace(
            "@EXECUTE@",
            if probe {
                r#"command printf %s "$cfg""#
            } else {
                TRANSFER
            },
        );
    let (setup, receiver) = script.split_once('\n').unwrap();
    let receiver = receiver.trim();
    let hash = Sha256::digest(receiver.as_bytes());
    // The known-good old bootstrap fits in a single ~1.7 KB input burst.
    // Wait for the protected child before uploading the second fixed line.
    // A host without sha256sum echoes this public code for exact comparison.
    let script = setup
        .trim_end()
        .replace("@LENGTH@", &receiver.len().to_string())
        .replace("@HASH@", &format!("{hash:x}"));
    format!(
        " sh -c '{}' {session}\n{receiver}\n",
        script.replace('\'', "'\\''")
    )
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Driver<'_> {
    async fn verify_fixed(&mut self, expected: &[u8], sid: &str) -> Result<(), &'static str> {
        let mut offset = 0;
        loop {
            match self.next().await? {
                Fragment::Data(bytes) if expected[offset..].starts_with(&bytes) => {
                    offset += bytes.len()
                }
                Fragment::Frame(marker)
                    if marker == format!("{sid}:VERIFY") && offset == expected.len() =>
                {
                    self.send(format!("OK:{sid}\n").as_bytes()).await?;
                    return Ok(());
                }
                _ => return Err("SSH_TERMINAL_PROTOCOL_ERROR"),
            }
        }
    }

    pub(super) async fn curl_input(
        &mut self,
        request: &TerminalCurlRequest,
        probe: bool,
        cancellation: &CancellationToken,
    ) -> Result<(), &'static str> {
        let started = Instant::now();
        let burst = (!request.needs_data_raw)
            .then(|| Burst::bounded(&request.config))
            .flatten();
        let burst_was_ineligible = burst.is_none();
        let previous = self.lease.curl_context.take();
        let script = bootstrap(
            &self.session,
            request.needs_data_raw,
            probe,
            burst.is_some(),
            previous.as_deref(),
        );
        let (setup, receiver) = script.split_once('\n').unwrap();
        self.send(format!("{setup}\n").as_bytes()).await?;
        let mut sent_receiver = false;
        let mut prompt = crate::ssh::login_prompt::LoginPrompt::default();
        let sid = self.session.clone();
        let prefix = format!("{sid}:");
        let (encoding, fast, context) = loop {
            let frame = self.handshake_frame(&mut prompt).await?;
            let Some(fields) = frame.strip_prefix(&prefix) else {
                continue;
            };
            if fields == "LOAD" && !sent_receiver {
                if cancellation.is_cancelled() {
                    self.child_ready = true;
                    return Err("SSH_TERMINAL_CANCELLED");
                }
                sent_receiver = true;
                self.child_ready = true;
                // PULSE acknowledges the first short line. A later empty line
                // terminates a public script truncated before LF by a relay.
                let public = if self.lease.short_curl_setup {
                    ""
                } else {
                    receiver.trim_end()
                };
                self.send(format!("+\n{public}\n").as_bytes()).await?;
                self.deadline.enter(Stage::Ready);
                continue;
            }
            if fields == "PULSE" && sent_receiver {
                self.send(b"\n").await?;
                continue;
            }
            if fields == "SHORT" && sent_receiver {
                self.lease.short_curl_setup = true;
                let mut length = 0;
                for part in receiver.trim_end().as_bytes().chunks(240) {
                    self.send(&[part, b"\n"].concat()).await?;
                    length += part.len();
                    self.frame(&format!("{sid}:L:{length}")).await?;
                }
                continue;
            }
            if fields == "BAD" {
                self.child_ready = true;
                return Err("SSH_TERMINAL_PROTOCOL_ERROR");
            }
            if fields == "CHECK" && sent_receiver {
                self.child_ready = true;
                self.verify_fixed(receiver.trim_end_matches('\n').as_bytes(), &sid)
                    .await?;
                continue;
            }
            if let Some(code) = fields
                .strip_prefix("PREP:")
                .or_else(|| fields.strip_prefix("FAIL:"))
            {
                self.child_exiting = fields.starts_with("PREP:");
                self.child_ready = !self.child_exiting;
                return Err(match code {
                    "123" => "SSH_TERMINAL_BASE64_MISSING",
                    "124" => "SSH_TERMINAL_HTTP_CURL_BODY_UNSUPPORTED",
                    "126" => "SSH_TERMINAL_PRINTF_UNSUPPORTED",
                    "127" => "SSH_TERMINAL_HTTP_CURL_UNAVAILABLE",
                    _ => "SSH_TERMINAL_HTTP_CURL_UNSUPPORTED",
                });
            }
            let Some(ready) = fields.strip_prefix("READY:4:") else {
                continue;
            };
            if !sent_receiver {
                return Err("SSH_TERMINAL_PROTOCOL_ERROR");
            }
            let mut caps = ready.split(':');
            let encoding = match caps.next() {
                Some("b64") => protocol::InputEncoding::Base64,
                Some("octal") => protocol::InputEncoding::Octal,
                _ => return Err("SSH_TERMINAL_PROTOCOL_ERROR"),
            };
            let context = caps.next().unwrap_or("");
            let fast = match caps.next() {
                Some("B")
                    if matches!(encoding, protocol::InputEncoding::Base64)
                        && valid_hash(context)
                        && burst.is_some()
                        && (probe || previous.as_deref() == Some(context)) =>
                {
                    true
                }
                Some("D") => false,
                _ => return Err("SSH_TERMINAL_PROTOCOL_ERROR"),
            };
            if caps.next().is_some() {
                return Err("SSH_TERMINAL_PROTOCOL_ERROR");
            }
            self.child_ready = true;
            break (encoding, fast, context.to_owned());
        };
        self.timing.prepare_ms = started.elapsed().as_millis();
        self.deadline.enter(Stage::InputAck);
        if cancellation.is_cancelled() {
            return Err("SSH_TERMINAL_CANCELLED");
        }
        if fast {
            let burst = burst.unwrap();
            let input = Instant::now();
            self.send(&burst.wire).await?;
            let waiting = Instant::now();
            self.input_frame(&sid, &format!("{sid}:INPUT_END:{}", burst.totals))
                .await?;
            self.timing.input_ack_wait_ms = waiting.elapsed().as_millis();
            self.timing.input_ack_count = 1;
            self.timing.input_ms = input.elapsed().as_millis();
            self.timing.input_wire_bytes = burst.wire.len();
            self.timing.input_burst = true;
        } else {
            self.confirm_input(encoding, &request.config, &sid, cancellation)
                .await?;
        }
        if !probe && burst_was_ineligible && !self.lease.short_curl_setup {
            // No identity was requested. The next eligible use checks it again.
            self.lease.curl_context = previous;
        } else if fast || (!probe && previous.as_deref() == Some(&context)) {
            self.lease.curl_context = Some(context);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "emit fixed scripts for real POSIX PTY QA"]
    fn emit_curl_bootstraps() {
        let dir = std::path::PathBuf::from(std::env::var_os("UNFOUR_CURL_QA_BOOTSTRAPS").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        for (name, raw, probe, context) in [
            ("confirmed", false, false, None),
            ("raw", true, false, None),
            ("burst", false, false, Some("a".repeat(64))),
            ("probe", false, true, None),
        ] {
            let script = bootstrap(&"0".repeat(32), raw, probe, true, context.as_deref());
            std::fs::write(dir.join(name), script).unwrap();
        }
    }

    #[test]
    fn actual_probe_frame_bounds_all_bytes_and_bootstrap_lines() {
        assert_eq!(
            Burst::bounded(&"x".repeat(SAMPLE_BYTES))
                .unwrap()
                .wire
                .len(),
            SAMPLE_WIRE_BYTES
        );
        assert!(Burst::bounded(&"x".repeat(SAMPLE_BYTES + 1)).is_none());
        for raw in [false, true] {
            let script = bootstrap(&"0".repeat(32), raw, false, true, Some(&"a".repeat(64)));
            let sizes: Vec<_> = script.split_inclusive('\n').map(str::len).collect();
            assert_eq!(sizes.len(), 2);
            assert!(sizes[0] <= 1697 && sizes[1] + 2 <= 1697, "{sizes:?}");
        }
    }
}
