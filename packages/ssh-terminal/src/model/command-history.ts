/**
 * Tracks the editable command line visible to a normal xterm buffer and serves
 * history suggestions for the typed prefix.
 *
 * This is deliberately conservative. If input contains an editing operation
 * whose resulting line cannot be known locally (for example Tab completion),
 * the next Enter is not reported for persistence and no suggestions are
 * offered for that line. The remote PTY still receives every byte unchanged.
 */

export type TerminalPromptContext = "shell" | "secret" | "other" | null;

export type TerminalCommandLineState = {
  line: string;
  cursorAtEnd: boolean;
  reliable: boolean;
};

const HISTORY_CAP = 200;
const MIN_SUGGEST_CHARS = 2;

export class TerminalCommandHistoryController {
  private chars: string[] = [];
  private cursor = 0;
  private escapeSequence = "";
  private history: string[] = [];
  private inPaste = false;
  private promptKind: TerminalPromptContext = null;
  private reliable = true;

  accept(data: string): string[] {
    const submitted: string[] = [];
    for (const char of data) {
      if (this.escapeSequence) {
        this.acceptEscapeCharacter(char);
        continue;
      }
      if (char === "\x1b") {
        this.escapeSequence = char;
        continue;
      }
      if (char === "\r" || char === "\n") {
        const command = this.acceptEnter();
        if (command !== null) submitted.push(command);
        continue;
      }
      if (this.acceptControlCharacter(char)) continue;
      this.chars.splice(this.cursor, 0, char);
      this.cursor += 1;
    }
    return submitted;
  }

  private acceptEnter(): string | null {
    if (this.inPaste) {
      // Inside a bracketed paste the newline is buffered by the remote line
      // editor instead of executing the line, so the block cannot be tracked
      // as individually executed commands.
      this.reliable = false;
      return null;
    }
    const command = this.chars.join("").trim();
    const record =
      command.length > 0 && this.reliable && this.promptKind !== "secret" ? command : null;
    if (record !== null) this.remember(record);
    this.resetCurrentLine();
    return record;
  }

  /** Handles single-byte line-editing controls; returns false for printable
   * characters so the caller inserts them at the cursor. */
  private acceptControlCharacter(char: string): boolean {
    switch (char) {
      case "\x03":
        this.resetCurrentLine();
        return true;
      case "\x01":
        this.cursor = 0;
        return true;
      case "\x05":
        this.cursor = this.chars.length;
        return true;
      case "\x0b":
        this.chars.splice(this.cursor);
        return true;
      case "\x15":
        this.replaceLine("");
        return true;
      case "\x17":
        this.deletePreviousWord();
        return true;
      case "\x08":
      case "\x7f":
        if (this.cursor > 0) {
          this.chars.splice(this.cursor - 1, 1);
          this.cursor -= 1;
        }
        return true;
      default:
        if (char === "\t" || char < " ") {
          // Completion and uncommon readline bindings can change the remote
          // line in ways xterm does not report back to the browser.
          this.reliable = false;
          return true;
        }
        return false;
    }
  }

  observeOutput(output: string) {
    const visible = stripControlAndAnsi(output);
    if (looksLikeSecretPrompt(visible)) {
      this.promptKind = "secret";
      return;
    }
    if (looksLikeShellPrompt(visible)) {
      this.promptKind = "shell";
      return;
    }
    // Output that scrolled a line means whatever prompt we sat on is gone.
    // Keystroke echo chunks carry no line break and leave the context alone.
    if (visible.includes("\n") || visible.includes("\r")) {
      this.promptKind = "other";
    }
  }

  promptContext(): TerminalPromptContext {
    return this.promptKind;
  }

  /** History suggestions for the current line: exact-prefix matches first
   * (newest first), then substring matches. An exact current-line match stays
   * visible so users can confirm that the command was recorded; accepting it
   * is a safe no-op handled by the suggestion consumer. */
  suggest(limit = 8): string[] {
    const line = this.currentLine().trim();
    if (line.length < MIN_SUGGEST_CHARS) return [];
    const lower = line.toLowerCase();
    const prefixMatches: string[] = [];
    const containsMatches: string[] = [];
    for (const command of this.history) {
      const commandLower = command.toLowerCase();
      if (commandLower.startsWith(lower)) {
        prefixMatches.push(command);
      } else if (commandLower.includes(lower)) {
        containsMatches.push(command);
      }
      if (prefixMatches.length >= limit) break;
    }
    return [...prefixMatches, ...containsMatches].slice(0, limit);
  }

  lineState(): TerminalCommandLineState {
    return {
      line: this.currentLine(),
      cursorAtEnd: this.cursor === this.chars.length,
      reliable: this.reliable,
    };
  }

  setHistory(commands: string[]) {
    const incoming = commands.filter((command) => command.trim().length > 0);
    const merged: string[] = [];
    for (const command of [...this.history, ...incoming]) {
      if (command.trim().length > 0 && !merged.includes(command)) merged.push(command);
    }
    this.history = merged.slice(0, HISTORY_CAP);
  }

  reset() {
    this.history = [];
    this.escapeSequence = "";
    this.inPaste = false;
    this.promptKind = null;
    this.resetCurrentLine();
  }

  resetCurrentLine() {
    this.chars = [];
    this.cursor = 0;
    this.reliable = true;
    this.escapeSequence = "";
  }

  currentLine() {
    return this.chars.join("");
  }

  private acceptEscapeCharacter(char: string) {
    this.escapeSequence += char;
    if (this.escapeSequence === "\x1b[") return;
    const complete = /[A-Za-z~]$/.test(this.escapeSequence);
    if (!complete) {
      if (this.escapeSequence.length > 16) {
        this.escapeSequence = "";
        this.reliable = false;
      }
      return;
    }

    const sequence = this.escapeSequence;
    this.escapeSequence = "";
    switch (sequence) {
      case "\x1b[D":
        this.cursor = Math.max(0, this.cursor - 1);
        break;
      case "\x1b[C":
        this.cursor = Math.min(this.chars.length, this.cursor + 1);
        break;
      case "\x1b[H":
      case "\x1b[1~":
        this.cursor = 0;
        break;
      case "\x1b[F":
      case "\x1b[4~":
        this.cursor = this.chars.length;
        break;
      case "\x1b[3~":
        if (this.cursor < this.chars.length) this.chars.splice(this.cursor, 1);
        break;
      case "\x1b[200~":
        this.inPaste = true;
        break;
      case "\x1b[201~":
        this.inPaste = false;
        break;
      default:
        // Arrow history recall or another escape reached the remote line
        // editor; the resulting line text may differ from our local model.
        this.reliable = false;
    }
  }

  private deletePreviousWord() {
    while (this.cursor > 0 && /\s/.test(this.chars[this.cursor - 1])) {
      this.chars.splice(this.cursor - 1, 1);
      this.cursor -= 1;
    }
    while (this.cursor > 0 && !/\s/.test(this.chars[this.cursor - 1])) {
      this.chars.splice(this.cursor - 1, 1);
      this.cursor -= 1;
    }
  }

  private replaceLine(value: string) {
    this.chars = Array.from(value);
    this.cursor = this.chars.length;
    this.reliable = true;
  }

  private remember(command: string) {
    if (this.history[0] !== command) this.history.unshift(command);
    if (this.history.length > HISTORY_CAP) this.history.length = HISTORY_CAP;
  }
}

function looksLikeSecretPrompt(visible: string) {
  return /(password|passphrase|passcode)[^:\n]{0,80}:/i.test(visible);
}

function looksLikeShellPrompt(visible: string) {
  return /(?:^|[\r\n])[^\r\n]*[#$%❯➜»] $/.test(visible);
}

/** Drop ANSI escape sequences and control characters (keeping line breaks and
 * tabs) so prompt detection sees what the user sees. Mirrors the conservative
 * escape scanning used by the Rust-side tracker. */
function stripControlAndAnsi(output: string): string {
  let visible = "";
  let escape = "";
  for (const character of output) {
    if (escape) {
      escape += character;
      if (escape === "\x1b[") continue;
      if (/[A-Za-z~]$/.test(character) || escape.length > 16) escape = "";
      continue;
    }
    if (character === "\x1b") {
      escape = character;
      continue;
    }
    if (character === "\r" || character === "\n" || character === "\t") {
      visible += character;
      continue;
    }
    if (character < " " || character === "\x7f") continue;
    visible += character;
  }
  return visible;
}
