import { describe, expect, it } from "vitest";
import { TerminalCommandHistoryController } from "./command-history";

describe("TerminalCommandHistoryController", () => {
  it("captures commands only when Enter is received", () => {
    const history = new TerminalCommandHistoryController();
    expect(history.accept("git ")).toEqual([]);
    expect(history.accept("status\r")).toEqual(["git status"]);
    expect(history.currentLine()).toBe("");
  });

  it("tracks common readline edits before recording", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("echo ac");
    history.accept("\x1b[Db");
    expect(history.accept("\r")).toEqual(["echo abc"]);

    history.accept("echo wrong");
    history.accept("\x15echo right");
    expect(history.accept("\r")).toEqual(["echo right"]);
  });

  it("supports plain multiline input and ignores empty lines", () => {
    const history = new TerminalCommandHistoryController();
    expect(history.accept("pwd\r\n\rwhoami\n")).toEqual(["pwd", "whoami"]);
  });

  it("does not record lines pasted inside bracketed-paste markers", () => {
    const history = new TerminalCommandHistoryController();
    // The remote line editor buffers the block; nothing has executed yet.
    expect(history.accept("\x1b[200~echo one\recho two\x1b[201~")).toEqual([]);
    // The Enter that follows submits a block we cannot model reliably.
    expect(history.accept("\r")).toEqual([]);
    // Single-line bracketed paste stays trackable.
    expect(history.accept("\x1b[200~git status\x1b[201~\r")).toEqual(["git status"]);
  });

  it("does not persist a line after unknown completion edits", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("cat pack\t");
    expect(history.accept("age.json\r")).toEqual([]);
    expect(history.accept("pwd\r")).toEqual(["pwd"]);
  });

  it("suggests prefix matches before substring matches, newest first", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory([
      "git status",
      "sudo systemctl restart nginx",
      "git push origin main",
      "tail -f /var/log/nginx/error.log",
    ]);

    history.accept("gi");
    // "nginx" also contains the substring "gi", so those entries rank after
    // the true prefix matches.
    expect(history.suggest()).toEqual([
      "git status",
      "git push origin main",
      "sudo systemctl restart nginx",
      "tail -f /var/log/nginx/error.log",
    ]);

    history.accept("\x15nginx");
    expect(history.suggest()).toEqual([
      "sudo systemctl restart nginx",
      "tail -f /var/log/nginx/error.log",
    ]);
  });

  it("requires two typed characters and excludes the exact current line", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["ls -la", "ls"]);

    history.accept("l");
    expect(history.suggest()).toEqual([]);

    history.accept("s");
    expect(history.suggest()).toEqual(["ls -la"]);
  });

  it("matches case-insensitively and honors the suggestion limit", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["GIT status", "git push", "git pull"]);
    history.accept("git");
    expect(history.suggest(2)).toEqual(["GIT status", "git push"]);
  });

  it("reports cursor position and reliability for suggestion gating", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("echo abc");
    expect(history.lineState()).toEqual({
      line: "echo abc",
      cursorAtEnd: true,
      reliable: true,
    });

    history.accept("\x1b[D");
    expect(history.lineState().cursorAtEnd).toBe(false);

    history.accept("\t");
    expect(history.lineState().reliable).toBe(false);
  });

  it("clears history when reset so a new connection cannot replay the old host", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["git status"]);
    history.reset();
    history.accept("gi");
    expect(history.suggest()).toEqual([]);
  });

  it("keeps commands remembered before setHistory arrives", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("lsblk\r");
    history.setHistory(["pwd", "git status"]);
    history.accept("ls");
    expect(history.suggest()).toEqual(["lsblk"]);
  });

  it("classifies shell, secret, and other prompt contexts from output", () => {
    const history = new TerminalCommandHistoryController();
    expect(history.promptContext()).toBeNull();

    history.observeOutput("Last login: Thu Aug 13\r\n");
    expect(history.promptContext()).toBe("other");

    // ANSI-colored prompt with a trailing reset still counts as a shell prompt.
    history.observeOutput("\x1b[32mdev@host\x1b[0m:\x1b[34m~\x1b[0m$ \x1b[0m");
    expect(history.promptContext()).toBe("shell");

    // Keystroke echo chunks carry no line break and keep the shell context.
    history.observeOutput("g");
    expect(history.promptContext()).toBe("shell");

    // A REPL banner scrolls lines without ending in a shell prompt.
    history.observeOutput("Python 3.12.0\r\n>>> ");
    expect(history.promptContext()).toBe("other");

    history.observeOutput("dev@host:~ ❯ ");
    expect(history.promptContext()).toBe("shell");

    history.observeOutput("[sudo] password for dev: ");
    expect(history.promptContext()).toBe("secret");
  });

  it("does not remember commands typed at a password prompt", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["git status"]);
    history.observeOutput("[sudo] password for dev: ");
    expect(history.accept("hunter2\r")).toEqual([]);
    history.observeOutput("dev@host:~$ ");
    expect(history.accept("pwd\r")).toEqual(["pwd"]);
    history.accept("pw");
    expect(history.suggest()).toEqual(["pwd"]);
  });
});
