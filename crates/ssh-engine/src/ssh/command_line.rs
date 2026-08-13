#[derive(Debug, Default)]
pub(super) struct SshCommandLineTracker {
    chars: Vec<char>,
    cursor: usize,
    escape_sequence: String,
    reliable: bool,
    echoed: usize,
    pending: Vec<String>,
}

impl SshCommandLineTracker {
    pub(super) fn new() -> Self {
        Self {
            reliable: true,
            ..Self::default()
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    /// Observe bytes already accepted by the PTY and return command lines that
    /// were fully echoed before Enter. Un-echoed lines stay pending until
    /// [`observe_output`] confirms a remote echo or drops them as secret input.
    pub(super) fn accept(&mut self, input: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for character in input.chars() {
            if !self.escape_sequence.is_empty() {
                self.accept_escape_character(character);
                continue;
            }
            match character {
                '\x1b' => self.escape_sequence.push(character),
                '\r' | '\n' => {
                    if let Some(command) = self.submit_line() {
                        commands.push(command);
                    }
                }
                '\x03' => self.reset_line(),
                '\x01' => self.cursor = 0,
                '\x05' => self.cursor = self.chars.len(),
                '\x0b' => {
                    self.chars.truncate(self.cursor);
                    self.clamp_echoed();
                }
                '\x15' => self.replace_line(""),
                '\x17' => self.delete_previous_word(),
                '\x08' | '\x7f' => {
                    if self.cursor > 0 {
                        self.chars.remove(self.cursor - 1);
                        self.cursor -= 1;
                        self.clamp_echoed();
                    }
                }
                '\t' => self.reliable = false,
                value if value.is_control() => self.reliable = false,
                value => {
                    self.chars.insert(self.cursor, value);
                    self.cursor += 1;
                    if self.cursor - 1 < self.echoed {
                        self.echoed = self.cursor - 1;
                    }
                }
            }
        }
        commands
    }

    /// Match PTY output against the current line and any pending Enter.
    /// A pending command is persisted only when its text is echoed; other
    /// visible output after Enter is treated as a non-echoed secret prompt.
    pub(super) fn observe_output(&mut self, output: &str) -> Vec<String> {
        let visible = visible_output(output);
        self.match_echo(&visible);

        let mut confirmed = Vec::new();
        let mut remaining = Vec::new();
        let has_other_visible = visible.chars().any(|character| !is_line_ending(character));
        for command in self.pending.drain(..) {
            if visible.contains(&command) {
                confirmed.push(command);
            } else if has_other_visible {
                // Echo-off prompts (sudo/ssh/mysql) typically emit a newline
                // plus a message without repeating the typed secret.
            } else {
                remaining.push(command);
            }
        }
        self.pending = remaining;
        confirmed
    }

    fn submit_line(&mut self) -> Option<String> {
        let command = self.chars.iter().collect::<String>().trim().to_string();
        let persist = if !command.is_empty() && self.reliable {
            if self.line_fully_echoed() {
                Some(command)
            } else {
                self.pending.push(command);
                None
            }
        } else {
            None
        };
        self.reset_line();
        persist
    }

    fn line_fully_echoed(&self) -> bool {
        !self.chars.is_empty() && self.echoed >= self.chars.len()
    }

    fn match_echo(&mut self, visible: &str) {
        for character in visible.chars() {
            if is_line_ending(character) {
                continue;
            }
            if self.echoed < self.chars.len() && self.chars[self.echoed] == character {
                self.echoed += 1;
            } else if self.echoed == 0 {
                continue;
            } else {
                break;
            }
        }
    }

    fn accept_escape_character(&mut self, character: char) {
        self.escape_sequence.push(character);
        if self.escape_sequence == "\x1b[" {
            return;
        }
        if !character.is_ascii_alphabetic() && character != '~' {
            if self.escape_sequence.len() > 16 {
                self.escape_sequence.clear();
                self.reliable = false;
            }
            return;
        }

        let sequence = std::mem::take(&mut self.escape_sequence);
        match sequence.as_str() {
            "\x1b[D" => self.cursor = self.cursor.saturating_sub(1),
            "\x1b[C" => self.cursor = (self.cursor + 1).min(self.chars.len()),
            "\x1b[H" | "\x1b[1~" => self.cursor = 0,
            "\x1b[F" | "\x1b[4~" => self.cursor = self.chars.len(),
            "\x1b[3~" => {
                if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                    self.clamp_echoed();
                }
            }
            "\x1b[200~" | "\x1b[201~" => {}
            _ => self.reliable = false,
        }
    }

    fn delete_previous_word(&mut self) {
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.chars.remove(self.cursor - 1);
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.chars.remove(self.cursor - 1);
            self.cursor -= 1;
        }
        self.clamp_echoed();
    }

    fn replace_line(&mut self, value: &str) {
        self.chars = value.chars().collect();
        self.cursor = self.chars.len();
        self.echoed = 0;
        self.reliable = true;
    }

    fn reset_line(&mut self) {
        self.chars.clear();
        self.cursor = 0;
        self.escape_sequence.clear();
        self.echoed = 0;
        self.reliable = true;
    }

    fn clamp_echoed(&mut self) {
        self.echoed = self.echoed.min(self.chars.len());
    }
}

fn is_line_ending(character: char) -> bool {
    character == '\n' || character == '\r'
}

fn visible_output(output: &str) -> String {
    let mut visible = String::new();
    let mut escape = String::new();
    for character in output.chars() {
        if !escape.is_empty() {
            escape.push(character);
            if escape == "\x1b[" {
                continue;
            }
            if character.is_ascii_alphabetic() || character == '~' {
                escape.clear();
            } else if escape.len() > 16 {
                escape.clear();
            }
            continue;
        }
        if character == '\x1b' {
            escape.push(character);
            continue;
        }
        if character.is_control() && !is_line_ending(character) && character != '\t' {
            continue;
        }
        visible.push(character);
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_commands_on_enter_and_tracks_edits() {
        let mut tracker = SshCommandLineTracker::new();
        assert!(tracker.accept("echo ac").is_empty());
        assert!(tracker.accept("\x1b[Db").is_empty());
        assert!(tracker.accept("\r").is_empty());
        assert_eq!(tracker.observe_output("echo abc\r\n"), vec!["echo abc"]);
        assert!(tracker.accept("one\rtwo\n\r").is_empty());
        assert_eq!(tracker.observe_output("one\r\ntwo\r\n"), vec!["one", "two"]);
    }

    #[test]
    fn ctrl_u_replacement_supports_persisted_history_recall() {
        let mut tracker = SshCommandLineTracker::new();
        tracker.accept("draft");
        tracker.observe_output("draft");
        tracker.accept("\x15git status");
        tracker.observe_output("git status");
        assert_eq!(tracker.accept("\r"), vec!["git status"]);
    }

    #[test]
    fn unknown_completion_skips_only_the_affected_line() {
        let mut tracker = SshCommandLineTracker::new();
        tracker.accept("cat pack\t");
        assert!(tracker.accept("age.json\r").is_empty());
        assert!(tracker.accept("pwd").is_empty());
        tracker.observe_output("pwd");
        assert_eq!(tracker.accept("\r"), vec!["pwd"]);
    }

    #[test]
    fn paste_without_echo_stays_pending_until_output_repeats_the_command() {
        let mut tracker = SshCommandLineTracker::new();
        assert!(tracker.accept("git status\r").is_empty());
        assert_eq!(tracker.observe_output("git status\r\n"), vec!["git status"]);
    }

    #[test]
    fn unechoed_secret_input_is_dropped_when_output_continues() {
        let mut tracker = SshCommandLineTracker::new();
        assert!(tracker.accept("hunter2\r").is_empty());
        assert!(tracker
            .observe_output("\r\nSorry, try again.\r\n")
            .is_empty());
        assert!(tracker.accept("pwd").is_empty());
        tracker.observe_output("pwd");
        assert_eq!(tracker.accept("\r"), vec!["pwd"]);
    }

    #[test]
    fn newline_only_output_does_not_drop_a_pending_command() {
        let mut tracker = SshCommandLineTracker::new();
        assert!(tracker.accept("ls\r").is_empty());
        assert!(tracker.observe_output("\r\n").is_empty());
        assert_eq!(tracker.observe_output("ls\r\nfile.txt\r\n"), vec!["ls"]);
    }

    #[test]
    fn reset_clears_pending_and_partial_input() {
        let mut tracker = SshCommandLineTracker::new();
        tracker.accept("rm -rf /tmp/foo");
        tracker.accept("secret\r");
        tracker.reset();
        assert!(tracker.accept("ls").is_empty());
        tracker.observe_output("ls");
        assert_eq!(tracker.accept("\r"), vec!["ls"]);
    }
}
