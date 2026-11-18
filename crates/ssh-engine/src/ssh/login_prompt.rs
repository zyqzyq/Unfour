//! Recognize a standalone login prompt across PTY reads without retaining terminal text.
pub(super) const PROMPTS: &[&[u8]] = &[
    b"login:",
    b"username:",
    b"user name:",
    b"password:",
    b"passphrase:",
];

#[derive(Default)]
pub(super) enum EscapeState {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

impl EscapeState {
    pub(super) fn visible(&mut self, byte: u8) -> bool {
        match (&*self, byte) {
            (EscapeState::Text, 0x1b) => *self = EscapeState::Escape,
            (EscapeState::Escape, b'[') => *self = EscapeState::Csi,
            (EscapeState::Escape, b']') => *self = EscapeState::Osc,
            (EscapeState::Escape, _) => *self = EscapeState::Text,
            (EscapeState::Csi, 0x40..=0x7e) => *self = EscapeState::Text,
            (EscapeState::Csi, _) => {}
            (EscapeState::Osc, 0x07) | (EscapeState::OscEscape, b'\\') => {
                *self = EscapeState::Text;
            }
            (EscapeState::Osc | EscapeState::OscEscape, 0x1b) => {
                *self = EscapeState::OscEscape;
            }
            (EscapeState::Osc | EscapeState::OscEscape, _) => {
                *self = EscapeState::Osc;
            }
            (EscapeState::Text, _) => return true,
        }
        false
    }
}

#[derive(Default)]
pub(super) struct LoginPrompt {
    escape: EscapeState,
    // Only store bytes that are a prefix of a fixed prompt. Other output is discarded.
    prefix: &'static [u8],
    skip_line: bool,
    active: bool,
}
impl LoginPrompt {
    /// The handshake must stop on a prompt even if later text in this read
    /// replaces it. The idle reader only needs the final state from active().
    pub(super) fn observe(&mut self, bytes: &[u8]) -> bool {
        let mut saw_prompt = false;
        for &byte in bytes {
            if !self.escape.visible(byte) {
                continue;
            }
            match byte {
                b'\r' | b'\n' => {
                    self.prefix = b"";
                    self.skip_line = false;
                }
                b' ' | b'\t' if self.prefix.is_empty() || self.active => {}
                b'\t' | 0x20..=0xff if !self.skip_line => {
                    let next = self.prefix.len();
                    if let Some(word) = PROMPTS.iter().find(|word| {
                        word.starts_with(self.prefix)
                            && word.get(next) == Some(&byte.to_ascii_lowercase())
                    }) {
                        self.prefix = &word[..=next];
                        self.active = self.prefix.len() == word.len();
                        saw_prompt |= self.active;
                        // A username echo or masked password still belongs to
                        // the same prompt until the terminal starts a new line.
                        self.skip_line = self.active;
                    } else {
                        self.prefix = b"";
                        self.active = false;
                        self.skip_line = true;
                    }
                }
                _ => {}
            }
        }
        saw_prompt
    }

    pub(super) fn active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_current_visible_login_prompt_blocks_input() {
        let mut prompt = LoginPrompt::default();
        prompt.observe(b"Last login: yesterday\r\n");
        prompt.observe(&[b'x'; 8192]);
        prompt.observe(b"\r\n\x1b]0;password:\x07\x1b[31m Pa");
        assert!(!prompt.active());
        prompt.observe(b"SSword:\x1b[0m \r\n");
        assert!(prompt.active());
        prompt.observe(b"  $ ");
        assert!(!prompt.active());
        prompt.observe(b"\r\nlog");
        prompt.observe(b"in: ");
        assert!(prompt.active());
        assert!(prompt.observe(b"\r\npassword:\r\nAnother line\r\n"));
        assert!(!prompt.active());
    }
}
