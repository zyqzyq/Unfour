// @vitest-environment jsdom
import type { ReactNode } from "react";
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SshConnection, SshSessionSummary } from "@unfour/command-client";
import { FeedbackProvider, I18nProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { SshConnectionsPage } from "./TerminalPage";
import { useTerminalStore } from "./model/terminal-state";

const mocks = vi.hoisted(() => ({
  listConnections: vi.fn(),
  listSessions: vi.fn(),
  connect: vi.fn(),
  close: vi.fn(),
  reset: vi.fn(),
  fingerprint: vi.fn(),
  history: vi.fn(),
  channel: vi.fn(),
}));

vi.mock("@unfour/command-client", () => ({
  listSshConnections: mocks.listConnections,
  listSshSessions: mocks.listSessions,
  connectSshSession: mocks.connect,
  closeSshSession: mocks.close,
  resetSshHostFingerprint: mocks.reset,
  getSshHostFingerprint: mocks.fingerprint,
  getSshSessionHistory: mocks.history,
  registerSshTerminalChannel: mocks.channel,
  saveSshConnection: vi.fn(),
  deleteSshConnection: vi.fn(),
  testSshConnection: vi.fn(),
  exportSshLog: vi.fn(),
}));

vi.mock("./components/TerminalWorkspace", () => ({
  TerminalWorkspace: ({
    onCloseSession,
    onNewSession,
    sessions,
  }: {
    onCloseSession: (sessionId: string) => void;
    onNewSession: () => void;
    sessions: Array<{ session: SshSessionSummary }>;
  }) => (
    <div>
      <button onClick={() => onNewSession()} type="button">Open session</button>
      {sessions.map((tab) => (
        <button
          key={tab.session.sessionId}
          onClick={() => onCloseSession(tab.session.sessionId)}
          type="button"
        >
          Close {tab.session.sessionId}
        </button>
      ))}
    </div>
  ),
}));

const connection: SshConnection = {
  id: "connection-1",
  workspaceId: "ws",
  name: "Fixture",
  host: "example.com",
  port: 22,
  username: "demo",
  authKind: "password",
  keyPath: null,
  credentialRef: "ref",
  createdAt: "2026-10-19T00:00:00.000Z",
  updatedAt: "2026-10-19T00:00:00.000Z",
  deletedAt: null,
  revision: 1,
  syncStatus: "local",
  remoteId: null,
};

function session(overrides: Partial<SshSessionSummary> = {}): SshSessionSummary {
  return {
    sessionId: "session-kept",
    workspaceId: "ws",
    connectionId: connection.id,
    status: "connected",
    reconnectAttempt: 0,
    authKind: "password",
    host: connection.host,
    username: connection.username,
    cols: 120,
    rows: 32,
    createdAt: "2026-10-19T00:00:00.000Z",
    updatedAt: "2026-10-19T00:00:01.000Z",
    warnings: ["ACTIVITY_RECORD_FAILED"],
    ...overrides,
  };
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  function Harness() {
    const [sidebar, setSidebar] = useState<ReactNode>(null);
    return (
      <QueryClientProvider client={client}>
        <I18nProvider initialLocale="en" storageKey="ssh-session-activity-test">
          <FeedbackProvider>
            {sidebar}
            <SshConnectionsPage onShellSidebarChange={setSidebar} workspaceId="ws" />
          </FeedbackProvider>
        </I18nProvider>
      </QueryClientProvider>
    );
  }
  return render(<Harness />);
}

beforeEach(() => {
  vi.clearAllMocks();
  useTerminalStore.getState().resetTerminalEvents();
  useWorkspaceStore.setState({ selectedSshConnectionId: connection.id });
  mocks.listConnections.mockResolvedValue([connection]);
  mocks.listSessions.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
  mocks.channel.mockResolvedValue(() => undefined);
  mocks.fingerprint.mockResolvedValue({
    workspaceId: "ws",
    host: connection.host,
    port: connection.port,
    fingerprint: "SHA256:trusted",
    createdAt: "2026-10-19T00:00:00.000Z",
  });
});

afterEach(cleanup);

it("keeps the connected session id when activity logging fails", async () => {
  mocks.connect.mockImplementation(async () => {
    const connected = session();
    mocks.listSessions.mockResolvedValue([connected]);
    return connected;
  });
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Connect" }));
  expect(await screen.findByText(/activity log could not be saved/i)).toBeVisible();
  await waitFor(() => expect(useTerminalStore.getState().activeSessionId).toBe("session-kept"));
  expect(useTerminalStore.getState().frontendFailedSessions).toEqual({});
  expect(mocks.connect).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "ws",
    connectionId: connection.id,
    cols: 120,
    rows: 32,
  });
});

it("reports a real connection failure without an activity warning", async () => {
  mocks.connect.mockRejectedValue(new Error("connection refused"));
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Connect" }));
  await waitFor(() => {
    const activeSessionId = useTerminalStore.getState().activeSessionId ?? "";
    expect(activeSessionId.startsWith("__frontend_failed_")).toBe(true);
  });
  expect(screen.queryByText(/activity log could not be saved/i)).not.toBeInTheDocument();
  expect(mocks.connect).toHaveBeenCalledOnce();
});

it("keeps a closed session when activity logging fails", async () => {
  const connected = session();
  mocks.listSessions.mockResolvedValue([connected]);
  mocks.close.mockResolvedValue(session({ status: "disconnected" }));
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Close session-kept" }));
  fireEvent.click(await screen.findByRole("button", { name: "Close Session" }));
  expect(await screen.findByText(/activity log could not be saved/i)).toBeVisible();
  expect(mocks.close).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "ws",
    sessionId: "session-kept",
  });
  expect(mocks.connect).not.toHaveBeenCalled();
});

it("resets a mismatched host key once and reconnects when activity logging fails", async () => {
  mocks.connect
    .mockRejectedValueOnce(new Error("host key verification failed: fingerprint does not match"))
    .mockImplementationOnce(async () => {
      const connected = session({ sessionId: "session-after-reset" });
      mocks.listSessions.mockResolvedValue([connected]);
      return connected;
    });
  mocks.reset.mockResolvedValue({ deleted: true, warnings: ["ACTIVITY_RECORD_FAILED"] });
  renderPage();
  expect(await screen.findByText("Fixture")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Open session" }));
  fireEvent.click(await screen.findByRole("button", { name: "Reset & Reconnect" }));
  await waitFor(() => {
    expect(screen.getAllByText(/activity log could not be saved/i).length).toBeGreaterThan(0);
  });
  await waitFor(() => expect(mocks.connect).toHaveBeenCalledTimes(2));
  expect(mocks.reset).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "ws",
    host: connection.host,
    port: connection.port,
  });
  await waitFor(() => {
    expect(useTerminalStore.getState().activeSessionId).toBe("session-after-reset");
  });
});
