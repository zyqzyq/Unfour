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

  it("supports multiline paste and ignores empty lines", () => {
    const history = new TerminalCommandHistoryController();
    expect(history.accept("pwd\r\n\rwhoami\n")).toEqual(["pwd", "whoami"]);
  });

  it("does not persist a line after unknown completion edits", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("cat pack\t");
    expect(history.accept("age.json\r")).toEqual([]);
    expect(history.accept("pwd\r")).toEqual(["pwd"]);
  });

  it("browses newest-first history and restores the draft", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["git status", "pwd"]);
    history.accept("draft");

    expect(history.previous()).toBe("git status");
    expect(history.previous()).toBe("pwd");
    expect(history.previous()).toBe("pwd");
    expect(history.next()).toBe("git status");
    expect(history.next()).toBe("draft");
    expect(history.next()).toBeUndefined();
  });

  it("clears recalled history when reset so a new connection cannot replay the old host", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["git status"]);
    history.reset();
    expect(history.previous()).toBeUndefined();
  });

  it("keeps commands remembered before setHistory arrives", () => {
    const history = new TerminalCommandHistoryController();
    history.accept("ls\r");
    history.setHistory(["pwd", "git status"]);
    expect(history.previous()).toBe("ls");
    expect(history.previous()).toBe("pwd");
  });

  it("does not remember or recall commands at a password prompt", () => {
    const history = new TerminalCommandHistoryController();
    history.setHistory(["git status"]);
    history.observeOutput("[sudo] password for dev: ");
    expect(history.blocksHistoryRecall()).toBe(true);
    expect(history.previous()).toBeUndefined();
    expect(history.accept("hunter2\r")).toEqual([]);
    history.observeOutput("dev@host:~$ ");
    expect(history.blocksHistoryRecall()).toBe(false);
    expect(history.accept("pwd\r")).toEqual(["pwd"]);
    expect(history.previous()).toBe("pwd");
  });
});
