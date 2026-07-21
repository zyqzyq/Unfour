// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import type { SshSessionSummary } from "@unfour/command-client";
import { SftpWorkspace } from "./SftpWorkspace";
import { DEFAULT_SFTP_PANEL_WIDTH, useSftpStore } from "../model/sftp-state";

const mocks = vi.hoisted(() => ({
  openSftp: vi.fn(),
  listDirectory: vi.fn(),
  listTransfers: vi.fn().mockResolvedValue([]),
}));

vi.mock("@unfour/command-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@unfour/command-client")>();
  return {
    ...actual,
    listSftpDirectory: (...args: unknown[]) => mocks.listDirectory(...args),
    listSftpTransfers: (...args: unknown[]) => mocks.listTransfers(...args),
    openSftp: (...args: unknown[]) => mocks.openSftp(...args),
    registerSftpTransferChannel: vi.fn(async () => () => undefined),
  };
});

function session(sessionId: string, connectionId: string): SshSessionSummary {
  const now = new Date().toISOString();
  return {
    sessionId,
    workspaceId: "workspace-1",
    connectionId,
    status: "connected",
    reconnectAttempt: 0,
    authKind: "password",
    host: `${connectionId}.example.test`,
    username: "demo",
    cols: 120,
    rows: 32,
    createdAt: now,
    updatedAt: now,
  };
}

function renderWorkspace(activeSession: SshSessionSummary, children: ReactNode = "PTY") {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <SftpWorkspace session={activeSession}>
        <div>{children}</div>
      </SftpWorkspace>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  mocks.openSftp.mockReset();
  mocks.listDirectory.mockReset();
  mocks.listTransfers.mockReset();
  mocks.listTransfers.mockResolvedValue([]);
  useSftpStore.setState({
    panelWidth: DEFAULT_SFTP_PANEL_WIDTH,
    tabs: {},
    transfers: {},
  });
  window.localStorage.clear();
});

describe("SftpWorkspace", () => {
  it("keeps the panel closed and does not initialize SFTP until the edge handle is clicked", async () => {
    const activeSession = session("session-a", "connection-a");
    const opened = {
      workspaceId: activeSession.workspaceId,
      sessionId: activeSession.sessionId,
      connectionId: activeSession.connectionId,
      homePath: "/home/demo",
    };
    let resolveOpen: ((value: typeof opened) => void) | undefined;
    mocks.openSftp.mockReturnValue(
      new Promise((resolve) => {
        resolveOpen = resolve;
      }),
    );
    mocks.listDirectory.mockResolvedValue({
      workspaceId: activeSession.workspaceId,
      sessionId: activeSession.sessionId,
      connectionId: activeSession.connectionId,
      path: "/home/demo",
      entries: [],
    });

    renderWorkspace(activeSession);
    expect(screen.queryByRole("complementary", { name: "Remote Files" })).toBeNull();
    expect(mocks.openSftp).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Open Remote Files panel" }));

    await waitFor(() => expect(mocks.openSftp).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("complementary", { name: "Remote Files" })).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize Remote Files panel" })).toHaveAttribute(
      "aria-valuemax",
      "960",
    );
    expect(screen.getByText("Loading remote files…")).toBeInTheDocument();
    await act(async () => resolveOpen?.(opened));
  });

  it("does not carry an open panel or directory request into another terminal tab", async () => {
    const first = session("session-a", "connection-a");
    const second = session("session-b", "connection-b");
    mocks.openSftp.mockImplementation(async ({ sessionId }: { sessionId: string }) => ({
      workspaceId: "workspace-1",
      sessionId,
      connectionId: sessionId === first.sessionId ? first.connectionId : second.connectionId,
      homePath: sessionId === first.sessionId ? "/srv/a" : "/srv/b",
    }));
    mocks.listDirectory.mockResolvedValue({
      workspaceId: "workspace-1",
      sessionId: first.sessionId,
      connectionId: first.connectionId,
      path: "/srv/a",
      entries: [],
    });

    const view = renderWorkspace(first);
    fireEvent.click(screen.getByRole("button", { name: "Open Remote Files panel" }));
    await waitFor(() => expect(mocks.openSftp).toHaveBeenCalledTimes(1));

    view.rerender(
      <QueryClientProvider client={new QueryClient()}>
        <SftpWorkspace session={second}>
          <div>PTY B</div>
        </SftpWorkspace>
      </QueryClientProvider>,
    );

    expect(screen.queryByRole("complementary", { name: "Remote Files" })).toBeNull();
    expect(screen.getByText("PTY B")).toBeInTheDocument();
    expect(mocks.openSftp).toHaveBeenCalledTimes(1);
  });

  it("keeps terminal content available when SFTP initialization fails", async () => {
    mocks.openSftp.mockRejectedValue(new Error("SFTP subsystem is disabled"));
    renderWorkspace(session("session-a", "connection-a"), "Terminal remains usable");

    fireEvent.click(screen.getByRole("button", { name: "Open Remote Files panel" }));

    await screen.findByText("SFTP subsystem is disabled");
    expect(screen.getByText("Terminal remains usable")).toBeInTheDocument();
  });
});
