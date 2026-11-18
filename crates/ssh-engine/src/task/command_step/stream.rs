use super::*;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const PROMPT_SCAN_BYTES: usize = 4 * 1024;

#[derive(Default)]
enum EscapeState {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

/// Retain an incomplete line until it can be redacted as one unit. Only fixed
/// diagnostic codes may bypass that rule; arbitrary partial text never does.
pub(in crate::ssh) struct StreamBuffer {
    total: usize,
    pub(in crate::ssh::task) truncated: bool,
    pending: Vec<u8>,
    escape: EscapeState,
    login_prompt: bool,
}

impl StreamBuffer {
    pub(in crate::ssh) fn new() -> Self {
        Self {
            total: 0,
            truncated: false,
            pending: Vec::new(),
            escape: EscapeState::Text,
            login_prompt: false,
        }
    }

    /// Observe protocol capabilities without exposing task events or raw output.
    pub(in crate::ssh) fn observe(&mut self, data: &[u8]) {
        self.push(data, "stdout", &mut |_| {});
    }

    pub(in crate::ssh::task) fn push(
        &mut self,
        data: &[u8],
        stream: &str,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) {
        if self.truncated || data.is_empty() {
            return;
        }
        let take = MAX_OUTPUT_BYTES.saturating_sub(self.total).min(data.len());
        self.truncated = take < data.len();
        for &byte in &data[..take] {
            self.total += 1;
            match (&self.escape, byte) {
                (EscapeState::Text, 0x1b) => self.escape = EscapeState::Escape,
                (EscapeState::Escape, b'[') => self.escape = EscapeState::Csi,
                (EscapeState::Escape, b']') => self.escape = EscapeState::Osc,
                (EscapeState::Escape, _) => self.escape = EscapeState::Text,
                (EscapeState::Csi, 0x40..=0x7e) => self.escape = EscapeState::Text,
                (EscapeState::Csi, _) => {}
                (EscapeState::Osc, 0x07) | (EscapeState::OscEscape, b'\\') => {
                    self.escape = EscapeState::Text;
                }
                (EscapeState::Osc | EscapeState::OscEscape, 0x1b) => {
                    self.escape = EscapeState::OscEscape;
                }
                (EscapeState::Osc | EscapeState::OscEscape, _) => {
                    self.escape = EscapeState::Osc;
                }
                (EscapeState::Text, b'\n') => {
                    self.login_prompt |= self.pending_is_prompt();
                    self.pending.push(byte);
                    self.flush(stream, emit);
                }
                (EscapeState::Text, b'\r' | b'\t' | 0x20..=0x7e | 0x80..=0xff) => {
                    self.pending.push(byte);
                }
                (EscapeState::Text, _) => {}
            }
        }
        if self.truncated {
            // A cut-off known secret may no longer match the later full-value
            // redactor. Discard the entire unfinished line, never its prefix.
            self.pending.clear();
        }
    }

    fn pending_is_prompt(&self) -> bool {
        if self.total > PROMPT_SCAN_BYTES || self.pending.len() > 128 {
            return false;
        }
        let prompt = String::from_utf8_lossy(&self.pending)
            .trim()
            .to_ascii_lowercase();
        // Require a whole short line, not an occurrence in a log/command.
        // Even a match is only an observation, never an execution verdict.
        matches!(
            prompt.as_str(),
            "login:" | "username:" | "user name:" | "password:" | "passphrase:"
        )
    }

    pub(in crate::ssh) fn has_login_prompt(&self) -> bool {
        self.login_prompt || self.pending_is_prompt()
    }

    pub(in crate::ssh::task) fn flush(
        &mut self,
        stream: &str,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            emit_redacted(stream, &String::from_utf8_lossy(&pending), emit);
        }
    }
}

pub(super) fn emit_diagnostic(code: &str, emit: &mut (dyn FnMut(DriverEvent) + Send)) {
    emit(DriverEvent::Output {
        stream: "diagnostic".into(),
        data: format!("{code}\n"),
    });
}

pub(in crate::ssh::task) fn emit_redacted(
    stream: &str,
    data: &str,
    emit: &mut (dyn FnMut(DriverEvent) + Send),
) {
    if data.is_empty() {
        return;
    }
    let (redacted, _) = unfour_core::redaction::redact_sensitive_lines(data);
    if !redacted.is_empty() {
        emit(DriverEvent::Output {
            stream: stream.to_string(),
            data: redacted,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_osc_and_login_prompt_are_recognized_without_leaking_partial_text() {
        let mut output = StreamBuffer::new();
        let mut events = Vec::new();
        for part in [
            b"\x1b]0;private-user@host".as_slice(),
            b"\x1b",
            b"\\\x1b[32mlo",
            b"gin:\x1b[0m ",
        ] {
            output.push(part, "stdout", &mut |event| events.push(event));
        }
        assert!(output.has_login_prompt());
        assert!(events.is_empty());
        output.flush("stdout", &mut |event| events.push(event));
        assert_eq!(
            events,
            vec![DriverEvent::Output {
                stream: "stdout".into(),
                data: "login: ".into()
            }]
        );
    }

    #[test]
    fn sensitive_lines_remain_whole_across_chunks_and_controls() {
        let mut output = StreamBuffer::new();
        let mut events = Vec::new();
        output.push(b"Author\x1b[31mization: Be", "stdout", &mut |e| {
            events.push(e)
        });
        assert!(events.is_empty());
        output.push(b"arer private-token\x1b[0m\n", "stdout", &mut |e| {
            events.push(e)
        });
        let formatted = format!("{events:?}");
        assert!(!formatted.contains("private-token"));
        assert!(!formatted.contains("Bearer"));
        assert!(formatted.contains(unfour_core::redaction::REDACTED_VALUE));
        assert!(!output.has_login_prompt());
    }

    #[test]
    fn only_standalone_early_prompt_is_observed() {
        for data in [
            b"last login: yesterday".as_slice(),
            b"result password: value",
            b"echo login:",
        ] {
            let mut output = StreamBuffer::new();
            output.push(data, "stdout", &mut |_| {});
            assert!(!output.has_login_prompt());
        }
        let mut output = StreamBuffer::new();
        output.push(&vec![b'\n'; PROMPT_SCAN_BYTES + 1], "stdout", &mut |_| {});
        output.push(b"login:", "stdout", &mut |_| {});
        assert!(!output.has_login_prompt());
    }

    #[test]
    fn utf8_survives_packet_boundaries_and_output_is_bounded() {
        let mut output = StreamBuffer::new();
        let mut events = Vec::new();
        for byte in "目录\n".as_bytes() {
            output.push(&[*byte], "stdout", &mut |e| events.push(e));
        }
        assert_eq!(
            events,
            vec![DriverEvent::Output {
                stream: "stdout".into(),
                data: "目录\n".into()
            }]
        );
        output.push(&vec![b'x'; MAX_OUTPUT_BYTES + 1], "stdout", &mut |_| {});
        assert!(output.truncated);
        assert_eq!(output.total, MAX_OUTPUT_BYTES);
    }

    #[test]
    fn truncated_partial_line_never_exposes_a_cut_off_secret() {
        let mut output = StreamBuffer::new();
        let mut events = Vec::new();
        output.push(b"complete safe line\n", "stdout", &mut |e| events.push(e));
        let mut large = vec![b'x'; MAX_OUTPUT_BYTES - 40];
        large.extend_from_slice(b"private-secret-prefix-and-remainder");
        output.push(&large, "stdout", &mut |e| events.push(e));
        output.flush("stdout", &mut |e| events.push(e));
        assert!(output.truncated);
        assert_eq!(
            events,
            vec![DriverEvent::Output {
                stream: "stdout".into(),
                data: "complete safe line\n".into()
            }]
        );
    }
}
