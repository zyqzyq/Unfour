// @vitest-environment jsdom
import type { ComponentProps } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { onlineManager, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SshConnection, SshSessionSummary } from "@unfour/command-client";
import { FeedbackProvider, I18nProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { SshConnectionsPage } from "./TerminalPage";
import type { TerminalWorkspace } from "./components/TerminalWorkspace";
import { useTerminalStore } from "./model/terminal-state";

const mocks = vi.hoisted(() => ({
  listConnections: vi.fn(),
  listSessions: vi.fn(),
  connect: vi.fn(),
  fingerprint: vi.fn(),
  history: vi.fn(),
  channel: vi.fn(),
  save: vi.fn(),
  test: vi.fn(),
}));

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSshConnections: mocks.listConnections,
  listSshSessions: mocks.listSessions,
  connectSshSession: mocks.connect,
  getSshHostFingerprint: mocks.fingerprint,
  getSshSessionHistory: mocks.history,
  registerSshTerminalChannel: mocks.channel,
  saveSshConnection: mocks.save,
  testSshConnection: mocks.test,
  listTemporaryRoutes: vi.fn().mockResolvedValue([]),
}));

vi.mock("./components/TerminalWorkspace", () => ({
  TerminalWorkspace: ({
    onNewConnection, onOpenPreferences, onNewSession, selectedConnection,
  }: ComponentProps<typeof TerminalWorkspace>) => (
    <div>
      <span>{selectedConnection?.name}</span>
      <button onClick={onNewConnection}>New fixture connection</button>
      <button onClick={() => onOpenPreferences()}>Edit fixture connection</button>
      <button onClick={onNewSession}>Open fixture session</button>
    </div>
  ),
}));

const connection: SshConnection = {
  id: "connection-1", workspaceId: "ws", name: "Fixture", host: "example.com",
  port: 22, username: "demo", authKind: "password", keyPath: null,
  credentialRef: " ref ", createdAt: "2026-10-19T00:00:00.000Z",
  updatedAt: "2026-10-19T00:00:00.000Z", deletedAt: null, revision: 1,
  syncStatus: "local", remoteId: null,
};

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  function Harness({ workspaceId }: { workspaceId: string }) {
    return (
      <QueryClientProvider client={client}>
        <I18nProvider initialLocale="en" storageKey="ssh-connection-flow-test">
          <FeedbackProvider>
            <SshConnectionsPage key={workspaceId} workspaceId={workspaceId} />
          </FeedbackProvider>
        </I18nProvider>
      </QueryClientProvider>
    );
  }
  const view = render(<Harness workspaceId="ws" />);
  return { ...view, client, switchWorkspace: (workspaceId: string) => view.rerender(<Harness workspaceId={workspaceId} />) };
}

async function editConnection() {
  await screen.findByText("Fixture");
  fireEvent.click(screen.getByRole("button", { name: "Edit fixture connection" }));
  return screen.findByRole("textbox", { name: "Name" });
}

function fillNewConnection() {
  fireEvent.click(screen.getByRole("button", { name: "New fixture connection" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "New fixture" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Host" }), { target: { value: "new.example.com" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Username" }), { target: { value: "new-user" } });
}

beforeEach(() => {
  vi.resetAllMocks();
  useTerminalStore.setState({ workspaceId: null, workspaceOrder: [], byWorkspace: {} });
  useTerminalStore.getState().resetTerminalEvents();
  useWorkspaceStore.setState({ selectedSshConnectionId: connection.id });
  mocks.listConnections.mockResolvedValue([connection]);
  mocks.listSessions.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
  mocks.channel.mockResolvedValue(() => undefined);
  mocks.fingerprint.mockResolvedValue(null);
  mocks.save.mockImplementation(async (input) => {
    const saved = { ...connection, ...input, id: input.id ?? "connection-new" };
    mocks.listConnections.mockResolvedValue([saved]);
    return saved;
  });
});

afterEach(cleanup);

it("creates a connection with the entered secret intact and selects the saved record", async () => {
  renderPage();
  await screen.findByText("Fixture");
  fillNewConnection();
  fireEvent.change(screen.getByLabelText(/^Password/), { target: { value: "  password with spaces  " } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  expect(mocks.save.mock.calls[0][0]).toMatchObject({
    workspaceId: "ws", name: "New fixture", host: "new.example.com", username: "new-user",
    secret: "  password with spaces  ", credentialRef: null, keyPath: null,
  });
  expect(mocks.save.mock.calls[0][0].id).toBeUndefined();
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(useWorkspaceStore.getState().selectedSshConnectionId).toBe("connection-new");
});

it("trims a private-key path while preserving the passphrase", async () => {
  renderPage();
  await screen.findByText("Fixture");
  fillNewConnection();
  fireEvent.click(screen.getByRole("radio", { name: "Private key" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Private key path" }), { target: { value: "  C:\\keys\\id_ed25519  " } });
  fireEvent.change(screen.getByLabelText(/^Key passphrase/), { target: { value: "  passphrase  " } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  expect(mocks.save.mock.calls[0][0]).toMatchObject({
    authKind: "private-key", keyPath: "C:\\keys\\id_ed25519", secret: "  passphrase  ",
  });
});

it("edits a saved connection with its conflict snapshot and keeps the saved secret", async () => {
  renderPage();
  const name = await editConnection();
  fireEvent.change(name, { target: { value: "Renamed fixture" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  expect(mocks.save.mock.calls[0][0]).toMatchObject({
    id: connection.id, workspaceId: "ws", name: "Renamed fixture",
    expectedSnapshot: connection, secret: null, credentialRef: "ref",
  });
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});

it("tests the full unsaved form through the test endpoint and retains the draft", async () => {
  mocks.test.mockResolvedValue({ ok: true, message: "Credentials accepted by fixture" });
  renderPage();
  await editConnection();
  fireEvent.change(screen.getByRole("textbox", { name: "Host" }), { target: { value: "edited.example.com" } });
  fireEvent.change(screen.getByLabelText(/^Password/), { target: { value: "replacement secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  expect(await screen.findByText("Credentials accepted by fixture")).toBeVisible();
  expect(mocks.test).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    id: connection.id, workspaceId: "ws", host: "edited.example.com",
    credentialRef: connection.credentialRef, secret: "replacement secret", expectedSnapshot: connection,
  }));
  expect(mocks.connect).not.toHaveBeenCalled();
  expect(mocks.save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.getByRole("textbox", { name: "Host" })).toHaveValue("edited.example.com");
  expect(screen.getByRole("button", { name: "Test connection" })).toBeEnabled();
});

it("tests a LAN host and saves locally while the WebView reports offline", async () => {
  mocks.test.mockResolvedValue({ ok: false, message: "LAN fixture unreachable" });
  renderPage();
  await editConnection();
  const online = onlineManager.isOnline();
  try {
    act(() => onlineManager.setOnline(false));
    const host = screen.getByRole("textbox", { name: "Host" });
    fireEvent.change(host, { target: { value: "fixture.lan" } });
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
    expect(await screen.findByText("LAN fixture unreachable")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(host).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mocks.save).toHaveBeenCalledOnce();
  } finally {
    act(() => onlineManager.setOnline(online));
  }
});

it("serializes test and save before repaint and allows correcting a failed test", async () => {
  let failTest!: (reason: Error) => void;
  mocks.test
    .mockReturnValueOnce(new Promise((_, reject) => { failTest = reject; }))
    .mockResolvedValueOnce({ ok: true, message: "corrected host accepted" });
  renderPage();
  await editConnection();
  const test = screen.getByRole("button", { name: "Test connection" });
  const form = screen.getByRole("button", { name: "Save" }).closest("form")!;
  act(() => {
    test.click();
    test.click();
    fireEvent.submit(form);
  });
  await waitFor(() => expect(mocks.test).toHaveBeenCalledOnce());
  expect(screen.getByRole("textbox", { name: "Host" })).toBeDisabled();
  expect(mocks.save).not.toHaveBeenCalled();
  await act(async () => { failTest(new Error("fixture authentication rejected")); });
  expect(await screen.findByText("fixture authentication rejected")).toBeVisible();
  expect(screen.getByRole("dialog", { name: /Connection failed/ })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Host" }), { target: { value: "corrected.example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  expect(await screen.findByText("corrected host accepted")).toBeVisible();
  expect(mocks.test.mock.lastCall?.[0]).toMatchObject({ host: "corrected.example.com" });
  expect(mocks.connect).not.toHaveBeenCalled();
  expect(useTerminalStore.getState().frontendFailedSessions).toEqual({});
});

it("keeps an old test completion out of a newly selected connection's editor", async () => {
  const other = { ...connection, id: "other", name: "Other fixture", host: "other.example.com" };
  let completeOld!: (result: { ok: boolean; message: string }) => void;
  let completeNew!: (result: { ok: boolean; message: string }) => void;
  mocks.listConnections.mockResolvedValue([connection, other]);
  mocks.test
    .mockReturnValueOnce(new Promise((resolve) => { completeOld = resolve; }))
    .mockReturnValueOnce(new Promise((resolve) => { completeNew = resolve; }));
  renderPage();
  await editConnection();
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(mocks.test).toHaveBeenCalledTimes(1));
  act(() => useWorkspaceStore.getState().setSelectedSshConnection(other.id));
  expect(screen.getByRole("textbox", { name: "Host" })).toHaveValue(other.host);
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(mocks.test).toHaveBeenCalledTimes(2));
  await act(async () => { completeOld({ ok: true, message: "old result" }); });
  expect(screen.queryByText("old result")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  await act(async () => { completeNew({ ok: true, message: "current result" }); });
  expect(await screen.findByText("current result")).toBeVisible();
});

it("waits for first host-key trust before connecting the selected connection", async () => {
  const connected: SshSessionSummary = {
    sessionId: "session-new", workspaceId: "ws", connectionId: connection.id,
    status: "connected", reconnectAttempt: 0, authKind: "password", host: connection.host,
    username: connection.username, cols: 120, rows: 32, createdAt: connection.createdAt,
    updatedAt: connection.updatedAt,
  };
  mocks.connect.mockImplementation(async () => {
    mocks.listSessions.mockResolvedValue([connected]);
    return connected;
  });
  renderPage();
  await screen.findByText("Fixture");
  fireEvent.click(screen.getByRole("button", { name: "Open fixture session" }));
  const confirm = await screen.findByRole("button", { name: "Trust & Connect" });
  expect(mocks.fingerprint).toHaveBeenCalledExactlyOnceWith({ workspaceId: "ws", host: connection.host, port: 22 });
  expect(mocks.connect).not.toHaveBeenCalled();
  fireEvent.click(confirm);
  await waitFor(() => expect(mocks.connect).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "ws", connectionId: connection.id, cols: 120, rows: 32,
  }));
  await waitFor(() => expect(useTerminalStore.getState().activeSessionId).toBe("session-new"));
});

it("updates the old workspace cache after unmount without closing the new workspace draft", async () => {
  let completeSave!: (connection: SshConnection) => void;
  mocks.save.mockReturnValue(new Promise<SshConnection>((resolve) => { completeSave = resolve; }));
  const { client, switchWorkspace } = renderPage();
  const name = await editConnection();
  fireEvent.change(name, { target: { value: "Saved later" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  const otherConnection = { ...connection, workspaceId: "other", id: "other-connection", name: "Other fixture" };
  mocks.listConnections.mockImplementation(async (workspaceId) => workspaceId === "other" ? [otherConnection] : [connection]);
  switchWorkspace("other");
  await screen.findByText("Other fixture");
  fillNewConnection();
  await act(async () => { completeSave({ ...connection, name: "Saved later" }); });
  await waitFor(() => expect(client.getQueryData<SshConnection[]>(["ssh-connections", "ws"])?.[0].name).toBe("Saved later"));
  expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("New fixture");
  expect(useWorkspaceStore.getState().selectedSshConnectionId).toBeNull();
  expect(client.getQueryData<SshConnection[]>(["ssh-connections", "other"])).toEqual([otherConnection]);
});
