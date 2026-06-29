// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SshSessionEvent, SshSessionSummary } from "@unfour/command-client";
import { sanitizeTerminalWriteChunk } from "../model/terminal-write-sanitizer";
import { TerminalPane } from "./TerminalPane";

const terminalState = vi.hoisted(() => ({
  cols: 120,
  rows: 32,
  dataHandlers: [] as Array<(data: string) => void>,
  resizeHandlers: [] as Array<(size: { cols: number; rows: number }) => void>,
  writes: [] as string[],
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(function TerminalMock() {
    return {
      get cols() {
        return terminalState.cols;
      },
      get rows() {
        return terminalState.rows;
      },
      attachCustomKeyEventHandler: vi.fn(),
      dispose: vi.fn(),
      focus: vi.fn(),
      getSelection: vi.fn(() => ""),
      hasSelection: vi.fn(() => false),
      loadAddon: vi.fn(),
      onData: vi.fn((handler: (data: string) => void) => {
        terminalState.dataHandlers.push(handler);
        return { dispose: vi.fn() };
      }),
      onResize: vi.fn((handler) => {
        terminalState.resizeHandlers.push(handler);
        return { dispose: vi.fn() };
      }),
      open: vi.fn((element: HTMLElement) => {
        terminalState.openElement = element;
      }),
      reset: vi.fn(),
      write: vi.fn((data: string) => terminalState.writes.push(data)),
    };
  }),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(function FitAddonMock() {
    return {
      fit: vi.fn(() => {
        terminalState.cols = 96;
        terminalState.rows = 28;
      }),
    };
  }),
}));

vi.mock("@xterm/addon-search", () => ({
  SearchAddon: vi.fn().mockImplementation(function SearchAddonMock() {
    return {};
  }),
}));

vi.mock("@unfour/command-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@unfour/command-client")>();
  return {
    ...actual,
    resizeSshSession: vi.fn().mockResolvedValue({}),
    sendSshInput: vi.fn(),
  };
});

vi.mock("@unfour/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@unfour/ui")>();
  return {
    ...actual,
    useI18n: () => ({ t: (key: string) => key }),
  };
});

import { resizeSshSession, sendSshInput } from "@unfour/command-client";

const resizeMock = vi.mocked(resizeSshSession);
const sendInputMock = vi.mocked(sendSshInput);

const session: SshSessionSummary = {
  authKind: "password",
  connectionId: "conn-1",
  createdAt: "2026-06-23T00:00:00.000Z",
  host: "example.test",
  id: "session-1",
  reconnectAttempt: 0,
  sessionId: "session-1",
  status: "connected",
  updatedAt: "2026-06-23T00:00:00.000Z",
  username: "dev",
  workspaceId: "ws-1",
  cols: 120,
  rows: 32,
};

describe("TerminalPane", () => {
  beforeEach(() => {
    terminalState.cols = 120;
    terminalState.rows = 32;
    terminalState.dataHandlers = [];
    terminalState.resizeHandlers = [];
    terminalState.writes = [];
    resizeMock.mockClear();
    sendInputMock.mockReset();
  });

  it("syncs the fitted terminal size to the SSH session even without an xterm resize event", async () => {
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
      expect(resizeMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        cols: 96,
        rows: 28,
      }),
    );
  });

  it("opens xterm in an unpadded fit host so padding cannot overflow the pane", () => {
    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    expect(terminalState.openElement).not.toBeNull();
    expect(terminalState.openElement).not.toHaveClass("p-2");
    expect(terminalState.openElement).toHaveClass("h-full", "w-full", "overflow-hidden");
    expect(terminalState.openElement?.parentElement).toHaveClass("p-2");
  });

  it("serializes terminal input chunks for interactive programs", async () => {
    let resolveFirst: (() => void) | null = null;
    sendInputMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = () =>
            resolve({
              sessionId: "session-1",
              kind: "output",
              data: "first accepted\r\n",
              createdAt: "2026-06-23T00:00:01.000Z",
            });
        }),
    );
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "next accepted\r\n",
      createdAt: "2026-06-23T00:00:02.000Z",
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

    terminalState.dataHandlers[0]?.("i");
    terminalState.dataHandlers[0]?.("hello");

    await waitFor(() => expect(sendInputMock).toHaveBeenCalledTimes(1));
    expect(sendInputMock).toHaveBeenLastCalledWith({
      workspaceId: "ws-1",
      sessionId: "session-1",
      data: "i",
    });

    resolveFirst?.();

    await waitFor(() => expect(sendInputMock).toHaveBeenCalledTimes(2));
    expect(sendInputMock).toHaveBeenLastCalledWith({
      workspaceId: "ws-1",
      sessionId: "session-1",
      data: "hello",
    });
  });
  it("writes appended output when a coalesced event grows", async () => {
    const firstEvent: SshSessionEvent = {
      sessionId: "session-1",
      kind: "output",
      data: "line 1\r\n",
      createdAt: "2026-06-23T00:00:01.000Z",
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
        events={[
          {
            ...firstEvent,
            data: "line 1\r\nline 2\r\n",
            createdAt: "2026-06-23T00:00:02.000Z",
          },
        ]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(terminalState.writes.some((data) => data.includes("line 2"))).toBe(true),
    );
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

