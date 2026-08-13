// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
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

describe("TerminalPane output rendering", () => {
  beforeEach(resetTerminalMocks);

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

  it("loads connection history and recalls it with Arrow Up and Down", async () => {
    listHistoryMock.mockResolvedValue([
      {
        id: "history-1",
        workspaceId: "ws-1",
        connectionId: "conn-1",
        sessionId: "session-old",
        command: "git status",
        cwd: null,
        exitCode: null,
        durationMs: null,
        redacted: false,
        executedAt: "2026-06-23T00:00:00.000Z",
      },
    ]);
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:04.000Z",
    });

    render(
      <TerminalPane
        active
        events={[]}
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
    const handler = terminalState.customKeyHandlers[0];
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(false);
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "\x15\x0bgit status",
      }),
    );
    sendInputMock.mockClear();

    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowDown" }))).toBe(false);
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "\x15\x0b",
      }),
    );
  });

  it("keeps a command typed before history finishes loading", async () => {
    let resolveHistory: (value: unknown[]) => void = () => undefined;
    listHistoryMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveHistory = resolve;
        }),
    );
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:04.000Z",
    });

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(terminalState.dataHandlers.length).toBeGreaterThan(0));
    terminalState.dataHandlers[0]?.("ls\r");
    resolveHistory([
      {
        id: "history-1",
        workspaceId: "ws-1",
        connectionId: "conn-1",
        sessionId: "session-old",
        command: "pwd",
        cwd: null,
        exitCode: null,
        durationMs: null,
        redacted: false,
        executedAt: "2026-06-23T00:00:00.000Z",
      },
    ]);

    await waitFor(() => expect(listHistoryMock).toHaveBeenCalled());
    const handler = terminalState.customKeyHandlers[0];
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(false);
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "\x15\x0bls",
      }),
    );
  });

  it("does not recall the previous connection while the next history list is in flight", async () => {
    listHistoryMock.mockResolvedValueOnce([
      {
        id: "history-1",
        workspaceId: "ws-1",
        connectionId: "conn-1",
        sessionId: "session-old",
        command: "git status",
        cwd: null,
        exitCode: null,
        durationMs: null,
        redacted: false,
        executedAt: "2026-06-23T00:00:00.000Z",
      },
    ]);
    sendInputMock.mockResolvedValue({
      sessionId: "session-2",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:04.000Z",
    });

    const { rerender } = render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(listHistoryMock).toHaveBeenCalledTimes(1));
    listHistoryMock.mockImplementation(() => new Promise(() => undefined));
    rerender(
      <TerminalPane
        active
        events={[]}
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
    const handler = terminalState.customKeyHandlers[0];
    sendInputMock.mockClear();
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

  it("does not keep the previous host history when the next list fails", async () => {
    listHistoryMock.mockResolvedValueOnce([
      {
        id: "history-1",
        workspaceId: "ws-1",
        connectionId: "conn-1",
        sessionId: "session-old",
        command: "git status",
        cwd: null,
        exitCode: null,
        durationMs: null,
        redacted: false,
        executedAt: "2026-06-23T00:00:00.000Z",
      },
    ]);
    const { rerender } = render(
      <TerminalPane
        active
        events={[]}
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
        events={[]}
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
    const handler = terminalState.customKeyHandlers[0];
    sendInputMock.mockClear();
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

  it("does not intercept Arrow Up at a password prompt", async () => {
    listHistoryMock.mockResolvedValue([
      {
        id: "history-1",
        workspaceId: "ws-1",
        connectionId: "conn-1",
        sessionId: "session-old",
        command: "git status",
        cwd: null,
        exitCode: null,
        durationMs: null,
        redacted: false,
        executedAt: "2026-06-23T00:00:00.000Z",
      },
    ]);
    render(
      <TerminalPane
        active
        events={[
          {
            sessionId: "session-1",
            kind: "output",
            data: "[sudo] password for dev: ",
            createdAt: "2026-06-23T00:00:01.000Z",
          },
        ]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() => expect(listHistoryMock).toHaveBeenCalled());
    await waitFor(() =>
      expect(terminalState.writes.some((chunk) => chunk.includes("password"))).toBe(true),
    );
    const handler = terminalState.customKeyHandlers[0];
    sendInputMock.mockClear();
    expect(handler?.(new KeyboardEvent("keydown", { key: "ArrowUp" }))).toBe(true);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

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
