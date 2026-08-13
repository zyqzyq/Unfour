import { vi } from "vitest";
import type { SshSessionSummary } from "@unfour/command-client";

const hoisted = vi.hoisted(() => ({
  terminalState: {
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
  },
}));

export const terminalState = hoisted.terminalState;

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

export const listHistoryMock = vi.mocked(listSshCommandHistory);
export const resizeMock = vi.mocked(resizeSshSession);
export const sendInputMock = vi.mocked(sendSshInput);
export const clipboardReadMock = vi.fn();
export const clipboardWriteMock = vi.fn();

export const session: SshSessionSummary = {
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

export function resetTerminalMocks() {
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
