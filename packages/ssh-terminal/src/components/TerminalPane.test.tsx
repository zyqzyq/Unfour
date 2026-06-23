// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SshSessionSummary } from "@unfour/command-client";
import { TerminalPane } from "./TerminalPane";

const terminalState = vi.hoisted(() => ({
  cols: 120,
  rows: 32,
  resizeHandlers: [] as Array<(size: { cols: number; rows: number }) => void>,
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
      onData: vi.fn(() => ({ dispose: vi.fn() })),
      onResize: vi.fn((handler) => {
        terminalState.resizeHandlers.push(handler);
        return { dispose: vi.fn() };
      }),
      open: vi.fn((element: HTMLElement) => {
        terminalState.openElement = element;
      }),
      reset: vi.fn(),
      write: vi.fn(),
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

import { resizeSshSession } from "@unfour/command-client";

const resizeMock = vi.mocked(resizeSshSession);

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
    terminalState.resizeHandlers = [];
    resizeMock.mockClear();
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
