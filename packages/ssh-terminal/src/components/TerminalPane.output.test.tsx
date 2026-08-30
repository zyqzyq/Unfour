// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { SshSessionEvent } from "@unfour/command-client";
import { sanitizeTerminalWriteChunk } from "../model/terminal-write-sanitizer";
import {
  listHistoryMock,
  resetTerminalMocks,
  resizeMock,
  sendInputMock,
  session,
  terminalState,
} from "./terminal-pane-test-harness";
import { TerminalPane } from "./TerminalPane";

const shellPromptEvent: SshSessionEvent = {
  sessionId: "session-1",
  kind: "output",
  data: "Last login: Thu Aug 13\r\ndev@host:~$ ",
  createdAt: "2026-06-23T00:00:01.000Z",
};

const replPromptEvent: SshSessionEvent = {
  sessionId: "session-1",
  kind: "output",
  data: "Python 3.12.0\r\n>>> ",
  createdAt: "2026-06-23T00:00:02.000Z",
};

const secretPromptEvent: SshSessionEvent = {
  sessionId: "session-1",
  kind: "output",
  data: "[sudo] password for dev: ",
  createdAt: "2026-06-23T00:00:02.000Z",
};

function historyEntry(id: string, command: string) {
  return {
    id,
    workspaceId: "ws-1",
    connectionId: "conn-1",
    sessionId: "session-old",
    command,
    cwd: null,
    exitCode: null,
    durationMs: null,
    redacted: false,
    executedAt: "2026-06-23T00:00:00.000Z",
  };
}

beforeEach(resetTerminalMocks);
// Popup assertions query the document; every group needs the same isolation.
afterEach(cleanup);

describe("TerminalPane output rendering", () => {

  it("writes only immutable events appended after the render cursor", async () => {
    const firstEvent: SshSessionEvent = {
      sessionId: "session-1",
      kind: "output",
      data: "line 1\r\n",
      createdAt: "2026-06-23T00:00:01.000Z",
    };
    const secondEvent: SshSessionEvent = {
      ...firstEvent,
      data: "line 2\r\n",
      createdAt: "2026-06-23T00:00:02.000Z",
    };
    const thirdEvent: SshSessionEvent = {
      ...firstEvent,
      data: "line 3\r\n",
      createdAt: "2026-06-23T00:00:03.000Z",
    };
    const { rerender } = render(
      <TerminalPane
        active
        events={[firstEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes.some((data) => data.includes("line 1"))).toBe(true),
    );
    terminalState.writes = [];

    rerender(
      <TerminalPane
        active
        events={[firstEvent, secondEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes).toEqual(["line 2\r\n"]),
    );

    terminalState.writes = [];
    rerender(
      <TerminalPane
        active
        events={[secondEvent, thirdEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(terminalState.writes).toEqual(["line 3\r\n"]));
  });

});

describe("TerminalPane history suggestions", () => {
  it("shows history suggestions while typing at a shell prompt and inserts with Tab", async () => {
    listHistoryMock.mockResolvedValue([
      historyEntry("history-1", "git status"),
      historyEntry("history-2", "git push origin main"),
    ]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(listHistoryMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        connectionId: "conn-1",
        limit: 100,
      }),
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );

    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    const options = await screen.findAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "git status",
      "git push origin main",
    ]);
    expect(options[0]?.getAttribute("aria-selected")).toBe("true");
    expect(sendInputMock).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      sessionId: "session-1",
      data: "gi",
    });

    const handler = terminalState.customKeyHandlers[0];
    act(() => {
      expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowDown" }))).toBe(false);
    });
    await waitFor(() =>
      expect(screen.getAllByRole("option")[1]?.getAttribute("aria-selected")).toBe("true"),
    );

    sendInputMock.mockClear();
    act(() => {
      expect(handler?.(new KeyboardEvent("keydown", { key: "Tab" }))).toBe(false);
    });
    // A prefix match only sends the missing suffix — no control characters.
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "t push origin main",
      }),
    );
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  });

  it("keeps multiple commands entered in one session available for suggestions", async () => {
    listHistoryMock.mockResolvedValue([]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );

    act(() => {
      terminalState.dataHandlers[0]?.("docker ps -a\r");
      terminalState.dataHandlers[0]?.("docker images\r");
      terminalState.dataHandlers[0]?.("docker");
    });

    const options = await screen.findAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "docker images",
      "docker ps -a",
    ]);
  });

  it("replaces the typed line with plain backspaces for substring matches", async () => {
    listHistoryMock.mockResolvedValue([
      historyEntry("history-1", "sudo systemctl restart nginx"),
    ]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );

    act(() => {
      terminalState.dataHandlers[0]?.("ngin");
    });
    await screen.findByRole("listbox");

    sendInputMock.mockClear();
    const handler = terminalState.customKeyHandlers[0];
    act(() => {
      expect(handler?.(new KeyboardEvent("keydown", { key: "Tab" }))).toBe(false);
    });
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: `${"\x7f".repeat(4)}sudo systemctl restart nginx`,
      }),
    );
  });

  it("dismisses suggestions with Escape until the next submitted line", async () => {
    listHistoryMock.mockResolvedValue([historyEntry("history-1", "git status")]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );

    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    await screen.findByRole("listbox");

    const handler = terminalState.customKeyHandlers[0];
    act(() => {
      expect(handler?.(new KeyboardEvent("keydown", { key: "Escape" }))).toBe(false);
    });
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());

    // Still suppressed while the same line keeps being edited.
    act(() => {
      terminalState.dataHandlers[0]?.("t");
    });
    expect(screen.queryByRole("listbox")).toBeNull();

    // Submitting the line re-arms suggestions for the next command.
    act(() => {
      terminalState.dataHandlers[0]?.("\r");
    });
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    await screen.findByRole("listbox");
  });

  it("keeps a command typed before history finishes loading", async () => {
    let resolveHistory: (value: unknown[]) => void = () => undefined;
    listHistoryMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveHistory = resolve;
        }),
    );

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(terminalState.dataHandlers.length).toBeGreaterThan(0));
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );
    act(() => {
      terminalState.dataHandlers[0]?.("lsblk\r");
    });
    act(() => {
      resolveHistory([historyEntry("history-1", "ls -la")]);
    });

    await waitFor(() => expect(listHistoryMock).toHaveBeenCalled());
    act(() => {
      terminalState.dataHandlers[0]?.("ls");
    });
    const options = await screen.findAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["lsblk", "ls -la"]);
  });

});

describe("TerminalPane native input boundaries", () => {
  it("passes arrows to the remote shell whenever no suggestions are visible", async () => {
    listHistoryMock.mockResolvedValue([historyEntry("history-1", "git status")]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );

    const handler = terminalState.customKeyHandlers[0];
    sendInputMock.mockClear();
    // Native shell history stays reachable at an empty prompt.
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowDown" }))).toBe(true);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

  it("keeps arrows native and suggestions closed in REPL-like contexts", async () => {
    listHistoryMock.mockResolvedValue([historyEntry("history-1", "git status")]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent, replPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes(">>>"))).toBe(true),
    );

    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    expect(screen.queryByRole("listbox")).toBeNull();
    const handler = terminalState.customKeyHandlers[0];
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
  });

  it("never intercepts keys while composing with an IME", async () => {
    listHistoryMock.mockResolvedValue([historyEntry("history-1", "git status")]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    await screen.findByRole("listbox");

    const handler = terminalState.customKeyHandlers[0];
    expect(
      handler?.(new KeyboardEvent("keydown", { key: "ArrowDown", isComposing: true })),
    ).toBe(true);
  });

  it("does not open suggestions at a password prompt", async () => {
    listHistoryMock.mockResolvedValue([historyEntry("history-1", "git status")]);

    render(
      <TerminalPane
        active
        events={[shellPromptEvent, secretPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("password"))).toBe(true),
    );
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    expect(screen.queryByRole("listbox")).toBeNull();
    const handler = terminalState.customKeyHandlers[0];
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
  });

  it("ignores keystrokes that cannot reach the PTY", async () => {
    listHistoryMock.mockResolvedValue([]);
    const { rerender } = render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly
        session={session}
      />,
    );
    await waitFor(() => expect(terminalState.dataHandlers.length).toBeGreaterThan(0));

    // Read-only keys are dropped: never sent, never tracked.
    act(() => {
      terminalState.dataHandlers[0]?.("phantom\r");
    });
    expect(sendInputMock).not.toHaveBeenCalled();

    rerender(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );
    // If the phantom line had been remembered, typing "ph" would suggest it.
    act(() => {
      terminalState.dataHandlers[0]?.("ph");
    });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

});

describe("TerminalPane history isolation", () => {
  it("clears suggestions from the previous connection while the next history is in flight", async () => {
    listHistoryMock.mockResolvedValueOnce([historyEntry("history-1", "git status")]);

    const { rerender } = render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() => expect(listHistoryMock).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    await screen.findByRole("listbox");

    listHistoryMock.mockImplementation(() => new Promise(() => undefined));
    rerender(
      <TerminalPane
        active
        events={[{ ...shellPromptEvent, sessionId: "session-2" }]}
        inputDisabled={false}
        readOnly={false}
        session={{
          ...session,
          connectionId: "conn-2",
          id: "session-2",
          sessionId: "session-2",
        }}
      />,
    );

    await waitFor(() => expect(listHistoryMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("does not keep the previous host history when the next list fails", async () => {
    listHistoryMock.mockResolvedValueOnce([historyEntry("history-1", "git status")]);
    const { rerender } = render(
      <TerminalPane
        active
        events={[shellPromptEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() => expect(listHistoryMock).toHaveBeenCalledTimes(1));

    listHistoryMock.mockRejectedValueOnce(new Error("history unavailable"));
    rerender(
      <TerminalPane
        active
        events={[{ ...shellPromptEvent, sessionId: "session-2" }]}
        inputDisabled={false}
        readOnly={false}
        session={{
          ...session,
          connectionId: "conn-2",
          id: "session-2",
          sessionId: "session-2",
        }}
      />,
    );
    await waitFor(() => expect(listHistoryMock).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("dev@host"))).toBe(true),
    );
    act(() => {
      terminalState.dataHandlers[0]?.("gi");
    });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

});

describe("TerminalPane output batching and resize", () => {
  it("combines a frame of output into one xterm write without refreshing per event", async () => {
    const firstEvent: SshSessionEvent = {
      sessionId: "session-1",
      kind: "output",
      data: "line 1\r\n",
      createdAt: "2026-06-23T00:00:01.000Z",
    };
    const secondEvent: SshSessionEvent = {
      ...firstEvent,
      data: "line 2\r\n",
      createdAt: "2026-06-23T00:00:02.000Z",
    };
    const thirdEvent: SshSessionEvent = {
      ...firstEvent,
      data: "line 3\r\n",
      createdAt: "2026-06-23T00:00:03.000Z",
    };
    const { rerender } = render(
      <TerminalPane
        active
        events={[firstEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );
    await waitFor(() => expect(terminalState.writes).toEqual(["line 1\r\n"]));
    await waitFor(() => expect(terminalState.refreshCalls).toBeGreaterThan(0));
    terminalState.writes = [];
    const refreshesBeforeAppend = terminalState.refreshCalls;

    rerender(
      <TerminalPane
        active
        events={[firstEvent, secondEvent, thirdEvent]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes).toEqual(["line 2\r\nline 3\r\n"]),
    );
    expect(terminalState.refreshCalls).toBe(refreshesBeforeAppend);
  });

  it("filters xterm request-mode sequences while preserving ordinary vi control output", () => {
    const sanitized = sanitizeTerminalWriteChunk(
      "\x1b[?25lA\x1b[?2026$pB\x1b[4$pC\x1b[46;1H",
    );

    expect(sanitized.value).toBe("\x1b[?25lABC\x1b[46;1H");
    expect(sanitized.removedSequences).toEqual(["\\x1b[?2026$p", "\\x1b[4$p"]);
  });

  it("resyncs the current terminal size when switching to a different SSH session", async () => {
    const { rerender } = render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(resizeMock).toHaveBeenCalledTimes(1));
    resizeMock.mockClear();

    rerender(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={{ ...session, id: "session-2", sessionId: "session-2" }}
      />,
    );

    await waitFor(() =>
      expect(resizeMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-2",
        cols: 96,
        rows: 28,
      }),
    );
  });
});
