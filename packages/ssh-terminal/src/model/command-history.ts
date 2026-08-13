/**
 * Tracks the editable command line visible to a normal xterm buffer.
 *
 * This is deliberately conservative. If input contains an editing operation
 * whose resulting line cannot be known locally (for example Tab completion),
 * the next Enter is not reported for persistence. The remote PTY still
 * receives every byte unchanged.
 */
export class TerminalCommandHistoryController {
  private chars: string[] = [];
  private cursor = 0;
  private draft = "";
  private escapeSequence = "";
  private history: string[] = [];
  private historyIndex = -1;
  private reliable = true;
  private secretPrompt = false;

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
        const command = this.chars.join("").trim();
        if (command && this.reliable && !this.secretPrompt) {
          submitted.push(command);
          this.remember(command);
        }
        this.resetCurrentLine();
        continue;
      }
      if (char === "\x03") {
        this.resetCurrentLine();
        continue;
      }
      if (char === "\x01") {
        this.cursor = 0;
        continue;
      }
      if (char === "\x05") {
        this.cursor = this.chars.length;
        continue;
      }
      if (char === "\x0b") {
        this.chars.splice(this.cursor);
        this.leaveHistoryNavigation();
        continue;
      }
      if (char === "\x15") {
        this.replaceLine("");
        continue;
      }
      if (char === "\x17") {
        this.deletePreviousWord();
        continue;
      }
      if (char === "\x08" || char === "\x7f") {
        if (this.cursor > 0) {
          this.chars.splice(this.cursor - 1, 1);
          this.cursor -= 1;
        }
        this.leaveHistoryNavigation();
        continue;
      }
      if (char === "\t" || char < " ") {
        // Completion and uncommon readline bindings can change the remote line
        // in ways xterm does not report back to the browser.
        this.reliable = false;
        this.leaveHistoryNavigation();
        continue;
      }

      this.chars.splice(this.cursor, 0, char);
      this.cursor += 1;
      this.leaveHistoryNavigation();
    }
    return submitted;
  }

  observeOutput(output: string) {
    if (looksLikeSecretPrompt(output)) this.secretPrompt = true;
    else if (looksLikeShellPrompt(output)) this.secretPrompt = false;
  }

  blocksHistoryRecall() {
    return this.secretPrompt;
  }

  setHistory(commands: string[]) {
    const incoming = commands.filter((command) => command.trim().length > 0);
    const merged: string[] = [];
    for (const command of [...this.history, ...incoming]) {
      if (command.trim().length > 0 && !merged.includes(command)) merged.push(command);
    }
    this.history = merged.slice(0, 200);
    this.historyIndex = -1;
    this.draft = "";
  }

  previous(): string | undefined {
    if (this.secretPrompt || this.history.length === 0) return undefined;
    if (this.historyIndex === -1) this.draft = this.chars.join("");
    if (this.historyIndex < this.history.length - 1) this.historyIndex += 1;
    const command = this.history[this.historyIndex];
    this.replaceLine(command);
    return command;
  }

  next(): string | undefined {
    if (this.secretPrompt || this.historyIndex === -1) return undefined;
    this.historyIndex -= 1;
    const command = this.historyIndex === -1 ? this.draft : this.history[this.historyIndex];
    this.replaceLine(command);
    return command;
  }

  reset() {
    this.history = [];
    this.historyIndex = -1;
    this.draft = "";
    this.escapeSequence = "";
    this.secretPrompt = false;
    this.resetCurrentLine();
  }

  resetCurrentLine() {
    this.chars = [];
    this.cursor = 0;
    this.reliable = true;
    this.historyIndex = -1;
    this.draft = "";
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
        this.leaveHistoryNavigation();
        break;
      case "\x1b[200~":
      case "\x1b[201~":
        // Bracketed-paste boundaries do not alter the command contents.
        break;
      default:
        // Up/Down are handled before xterm emits data when persisted history
        // is available. If another escape reaches this parser, avoid storing a
        // command whose final text may differ from our local model.
        this.reliable = false;
        this.leaveHistoryNavigation();
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
    this.leaveHistoryNavigation();
  }

  private leaveHistoryNavigation() {
    if (this.historyIndex !== -1) {
      this.historyIndex = -1;
      this.draft = "";
    }
  }

  private replaceLine(value: string) {
    this.chars = Array.from(value);
    this.cursor = this.chars.length;
    this.reliable = true;
  }

  private remember(command: string) {
    if (this.history[0] !== command) this.history.unshift(command);
    if (this.history.length > 200) this.history.length = 200;
  }
}

function looksLikeSecretPrompt(output: string) {
  return /(password|passphrase|passcode)[^:\n]{0,80}:/i.test(output);
}

function looksLikeShellPrompt(output: string) {
  return /(?:^|\r|\n)[^\r\n]*[#$%] $/.test(output);
}
