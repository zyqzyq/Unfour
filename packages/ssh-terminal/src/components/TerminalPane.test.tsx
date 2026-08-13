// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SshSessionEvent, SshSessionSummary } from "@unfour/command-client";
import { sanitizeTerminalWriteChunk } from "../model/terminal-write-sanitizer";
import { TerminalPane } from "./TerminalPane";

const terminalState = vi.hoisted(() => ({
  cols: 120,
  customKeyHandlerRegistrations: 0,
  customKeyHandlers: [] as Array<(event: KeyboardEvent) => boolean>,
  rows: 32,
  dataHandlers: [] as Array<(data: string) => void>,
  inputElement: null as HTMLTextAreaElement | null,
  openElement: null as HTMLElement | null,
  pasteCalls: [] as string[],
  refreshCalls: 0,
  resizeHandlers: [] as Array<(size: { cols: number; rows: number }) => void>,
  selectAllCalls: 0,
  selection: "",
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
      attachCustomKeyEventHandler: vi.fn((handler: (event: KeyboardEvent) => boolean) => {
        terminalState.customKeyHandlerRegistrations += 1;
        terminalState.customKeyHandlers.push(handler);
      }),
      buffer: { active: { type: "normal" } },
      dispose: vi.fn(),
      focus: vi.fn(),
      getSelection: vi.fn(() => terminalState.selection),
      hasSelection: vi.fn(() => Boolean(terminalState.selection)),
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
        const input = document.createElement("textarea");
        element.appendChild(input);
        terminalState.inputElement = input;
        terminalState.openElement = element;
      }),
      paste: vi.fn((data: string) => {
        terminalState.pasteCalls.push(data);
        terminalState.dataHandlers.forEach((handler) => handler(data));
      }),
      refresh: vi.fn(() => {
        terminalState.refreshCalls += 1;
      }),
      reset: vi.fn(),
      selectAll: vi.fn(() => {
        terminalState.selectAllCalls += 1;
      }),
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
    listSshCommandHistory: vi.fn().mockResolvedValue([]),
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

import {
  listSshCommandHistory,
  resizeSshSession,
  sendSshInput,
} from "@unfour/command-client";

const listHistoryMock = vi.mocked(listSshCommandHistory);
const resizeMock = vi.mocked(resizeSshSession);
const sendInputMock = vi.mocked(sendSshInput);
const clipboardReadMock = vi.fn();
const clipboardWriteMock = vi.fn();

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

function resetTerminalMocks() {
  terminalState.cols = 120;
  terminalState.customKeyHandlerRegistrations = 0;
  terminalState.customKeyHandlers = [];
  terminalState.rows = 32;
  terminalState.dataHandlers = [];
  terminalState.inputElement = null;
  terminalState.openElement = null;
  terminalState.pasteCalls = [];
  terminalState.refreshCalls = 0;
  terminalState.resizeHandlers = [];
  terminalState.selectAllCalls = 0;
  terminalState.selection = "";
  terminalState.writes = [];
  clipboardReadMock.mockReset();
  clipboardWriteMock.mockReset();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      readText: clipboardReadMock,
      writeText: clipboardWriteMock,
    },
  });
  resizeMock.mockClear();
  listHistoryMock.mockReset();
  listHistoryMock.mockResolvedValue([]);
  sendInputMock.mockReset();
  sendInputMock.mockResolvedValue({
    sessionId: "session-1",
    kind: "output",
    data: "",
    createdAt: "2026-06-23T00:00:04.000Z",
  });
}

describe("TerminalPane", () => {
  beforeEach(resetTerminalMocks);

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
});

describe("TerminalPane clipboard interactions", () => {
  beforeEach(resetTerminalMocks);

  it("copies the terminal selection from the custom context menu", async () => {
    terminalState.selection = "selected terminal output";
    clipboardWriteMock.mockResolvedValue(undefined);

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(
      screen.getByRole("menuitem", { name: /ssh\.actions\.copySelection/ }),
    );

    await waitFor(() =>
      expect(clipboardWriteMock).toHaveBeenCalledWith("selected terminal output"),
    );
    expect(clipboardWriteMock).toHaveBeenCalledTimes(1);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

  it("replaces the native menu and pastes the terminal selection exactly once", async () => {
    terminalState.selection = "selected terminal output";
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:03.000Z",
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

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(
      screen.getByRole("menuitem", { name: "ssh.actions.pasteSelection" }),
    );

    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "selected terminal output",
      }),
    );
    expect(terminalState.pasteCalls).toEqual(["selected terminal output"]);
    expect(sendInputMock).toHaveBeenCalledTimes(1);
  });
  it("disables selection paste without a selection and supports clipboard paste and select all", async () => {
    clipboardReadMock.mockResolvedValue("pasted command\r");
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:03.000Z",
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

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    expect(
      screen.getByRole("menuitem", { name: "ssh.actions.pasteSelection" }),
    ).toHaveAttribute("data-disabled");

    fireEvent.click(
      screen.getByRole("menuitem", { name: /ssh\.actions\.pasteClipboard/ }),
    );
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "pasted command\r",
      }),
    );
    expect(terminalState.pasteCalls).toEqual(["pasted command\r"]);
    expect(sendInputMock).toHaveBeenCalledTimes(1);

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(screen.getByRole("menuitem", { name: "ssh.actions.selectAll" }));
    expect(terminalState.selectAllCalls).toBe(1);
  });
  it("leaves Ctrl+V to xterm's single native paste path", async () => {
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

    expect(terminalState.customKeyHandlerRegistrations).toBe(1);
    expect(
      terminalState.customKeyHandlers[0]?.(
        new KeyboardEvent("keydown", { key: "v", ctrlKey: true }),
      ),
    ).toBe(true);
    terminalState.dataHandlers[0]?.("native paste once");

    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "native paste once",
      }),
    );
    expect(sendInputMock).toHaveBeenCalledTimes(1);
  });
});

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

