use super::super::command_step::stream::StreamBuffer;
use super::*;

/// Complete lines make task-secret redaction robust across SSH packet splits.
/// Oversized partial lines are dropped whole rather than leaking a secret prefix.
pub(super) struct Output {
    stream: StreamBuffer,
    warned: bool,
}
impl Default for Output {
    fn default() -> Self {
        Self {
            stream: StreamBuffer::new(),
            warned: false,
        }
    }
}
impl Output {
    pub(super) fn push(&mut self, data: &[u8], emit: &mut (dyn FnMut(DriverEvent) + Send)) {
        self.stream.push(data, "stdout", emit);
        if self.stream.truncated && !self.warned {
            self.warned = true;
            emit(DriverEvent::Output {
                stream: "stdout".into(),
                data: "\n[command output truncated]\n".into(),
            });
        }
    }
    pub(super) fn finish(&mut self, emit: &mut (dyn FnMut(DriverEvent) + Send)) {
        self.stream.flush("stdout", emit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ansi_sensitive_header_never_reaches_live_output() {
        let mut output = Output::default();
        let mut events = Vec::new();
        output.push(b"\x1b]0;private-title\x07Author\x1b[", &mut |event| {
            events.push(event)
        });
        output.push(
            b"31mization: Bearer private-secret\x1b[0m\n",
            &mut |event| events.push(event),
        );
        output.finish(&mut |event| events.push(event));
        assert_eq!(
            events,
            vec![DriverEvent::Output {
                stream: "stdout".into(),
                data: format!("{}\n", unfour_core::redaction::REDACTED_VALUE)
            }]
        );
    }
}
