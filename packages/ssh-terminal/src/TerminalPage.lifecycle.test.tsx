// @vitest-environment jsdom
import type { ComponentProps, ReactElement, ReactNode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SshConnection, SshSessionSummary, SshTerminalDataPayload } from "@unfour/command-client";
import { FeedbackProvider, I18nProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { SshConnectionsPage } from "./TerminalPage";
import type { SshConnectionTree } from "./components/SshConnectionTree";
import type { TerminalWorkspace } from "./components/TerminalWorkspace";
import { useTerminalStore } from "./model/terminal-state";
import { useSftpStore } from "./model/sftp-state";

const mocks = vi.hoisted(() => ({
  connections: vi.fn(), sessions: vi.fn(), connect: vi.fn(), close: vi.fn(),
  channel: vi.fn(), disposeChannel: vi.fn(), history: vi.fn(), workspace: vi.fn(), fingerprint: vi.fn(),
}));

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSshConnections: mocks.connections,
  listSshSessions: mocks.sessions,
  connectSshSession: mocks.connect,
  closeSshSession: mocks.close,
  registerSshTerminalChannel: mocks.channel,
  getSshSessionHistory: mocks.history,
  getSshHostFingerprint: mocks.fingerprint,
}));
vi.mock("./components/TerminalWorkspace", () => ({
  TerminalWorkspace: (props: ComponentProps<typeof TerminalWorkspace>) => {
    mocks.workspace(props);
    return <div data-testid="terminal-workspace" />;
  },
}));
vi.mock("./components/SshConnectionTree", () => ({ SshConnectionTree: () => null }));

function connection(workspaceId: string): SshConnection {
  return {
    id: `${workspaceId}-connection`, workspaceId, name: workspaceId, host: `${workspaceId}.example`,
    port: 22, username: "demo", authKind: "password", keyPath: null, credentialRef: "ref",
    createdAt: "2026-11-15T00:00:00Z", updatedAt: "2026-11-15T00:00:00Z",
    deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
  };
}
function session(workspaceId: string, overrides: Partial<SshSessionSummary> = {}): SshSessionSummary {
  return {
    sessionId: `${workspaceId}-session`, workspaceId, connectionId: connection(workspaceId).id,
    status: "connected", reconnectAttempt: 0, authKind: "password", host: `${workspaceId}.example`,
    username: "demo", cols: 120, rows: 32,
    createdAt: "2026-11-15T00:00:00Z", updatedAt: "2026-11-15T00:00:00Z", ...overrides,
  };
}
function event(workspaceId: string, data = `${workspaceId} output`) {
  return { sessionId: `${workspaceId}-session`, kind: "output" as const, data, createdAt: "2026-11-15T00:00:01Z" };
}
function workspaceProps(): ComponentProps<typeof TerminalWorkspace> {
  return mocks.workspace.mock.lastCall![0];
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
function mountPage(initial: Partial<ComponentProps<typeof SshConnectionsPage>> = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  for (const id of ["one", "two"]) {
    client.setQueryData(["ssh-connections", id], [connection(id)]);
    client.setQueryData(["ssh-sessions", id], [session(id)]);
  }
  let props = { workspaceId: "one", ...initial };
  function view() {
    return <QueryClientProvider client={client}>
      <I18nProvider initialLocale="en" storageKey="terminal-lifecycle-test">
        <FeedbackProvider><SshConnectionsPage key={props.workspaceId} {...props} /></FeedbackProvider>
      </I18nProvider>
    </QueryClientProvider>;
  }
  const mounted = render(view());
  return {
    ...mounted, client,
    update(patch: Partial<ComponentProps<typeof SshConnectionsPage>>) {
      props = { ...props, ...patch };
      mounted.rerender(view());
    },
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.connect.mockReset();
  mocks.close.mockReset();
  useTerminalStore.getState().resetTerminalEvents();
  useTerminalStore.setState({ workspaceId: null, byWorkspace: {}, workspaceOrder: [], searchOpen: false, splitMode: "single" });
  useTerminalStore.getState().activateWorkspace("one");
  useTerminalStore.getState().startTerminalSession("one-session", [event("one")]);
  useSftpStore.setState({ tabs: {}, transfers: {} });
  useWorkspaceStore.setState({ selectedSshConnectionId: "one-connection" });
  mocks.connections.mockImplementation(async (id: string) => [connection(id)]);
  mocks.sessions.mockImplementation(async (id: string) => [session(id)]);
  mocks.history.mockResolvedValue([]);
  mocks.channel.mockResolvedValue(mocks.disposeChannel);
  mocks.fingerprint.mockResolvedValue({ fingerprint: "SHA256:trusted" });
});
afterEach(cleanup);

it("isolates terminal state across keyed workspace changes and restores the previous workspace", async () => {
  const page = mountPage();
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("one-session"));
  page.update({ workspaceId: "two" });
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("two-session"));
  expect(workspaceProps().sessions.map(({ session: item }) => item.workspaceId)).toEqual(["two"]);
  expect(workspaceProps().events).toEqual([]);
  act(() => useTerminalStore.getState().appendTerminalEvents([event("two")]));
  page.update({ workspaceId: "one" });
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("one-session"));
  expect(workspaceProps().events).toEqual([event("one")]);
  expect(useTerminalStore.getState().byWorkspace.two.terminalEvents).toEqual([event("two")]);
});

it("ignores an old workspace connect completion after the page remounts", async () => {
  const pending = deferred<SshSessionSummary>();
  mocks.connect.mockReturnValueOnce(pending.promise);
  const page = mountPage();
  const invalidate = vi.spyOn(page.client, "invalidateQueries");
  act(() => workspaceProps().onDuplicate("one-connection", "one-session"));
  await waitFor(() => expect(mocks.connect).toHaveBeenCalled());
  page.update({ workspaceId: "two" });
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("two-session"));
  await act(async () => pending.resolve(session("one", { sessionId: "late-session" })));
  await waitFor(() => expect(invalidate).toHaveBeenCalledWith({ queryKey: ["ssh-sessions", "one"] }));
  expect(workspaceProps().activeSessionId).toBe("two-session");
  expect(useTerminalStore.getState().terminalEvents).toEqual([]);
  expect(useTerminalStore.getState().frontendFailedSessions).toEqual({});
});

it("cannot create a frontend failure from a rejected connect after a keyed workspace switch", async () => {
  const pending = deferred<SshSessionSummary>();
  mocks.connect.mockReturnValueOnce(pending.promise);
  const page = mountPage();
  act(() => workspaceProps().onNewSession());
  await waitFor(() => expect(mocks.connect).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "one", connectionId: "one-connection", cols: 120, rows: 32,
  }));
  page.update({ workspaceId: "two" });
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("two-session"));
  await act(async () => pending.reject(new Error("late connection refused")));
  expect(useTerminalStore.getState().frontendFailedSessions).toEqual({});
  expect(useTerminalStore.getState().terminalEvents).toEqual([]);
  expect(workspaceProps().activeSessionId).toBe("two-session");
  expect(useTerminalStore.getState().byWorkspace.one.frontendFailedSessions).toEqual({});
});

it("preserves the source session and SFTP after reconnect failure, then activates a successful replacement before cleanup", async () => {
  const failedAttempt = deferred<SshSessionSummary>();
  const retry = deferred<SshSessionSummary>();
  mocks.connect.mockReturnValueOnce(failedAttempt.promise).mockReturnValueOnce(retry.promise);
  useSftpStore.getState().setPanelOpen("one-session", "one-connection", true);
  useSftpStore.getState().setPanelPath("one-session", "one-connection", "/drafts");
  const originalSftp = useSftpStore.getState().tabs["one-session"];
  const page = mountPage();
  act(() => workspaceProps().onReconnect("one-session"));
  await waitFor(() => expect(mocks.connect).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "one", connectionId: "one-connection", cols: 120, rows: 32,
  }, "one-session"));
  expect(mocks.close).not.toHaveBeenCalled();
  await act(async () => failedAttempt.reject(new Error("reconnect refused")));
  await waitFor(() => expect(workspaceProps().actionError).toEqual(new Error("reconnect refused")));
  expect(workspaceProps().activeSessionId).toBe("one-session");
  expect(useTerminalStore.getState().frontendFailedSessions).toEqual({});
  expect(useTerminalStore.getState().terminalEvents).toEqual([event("one")]);
  expect(useSftpStore.getState().tabs["one-session"]).toBe(originalSftp);
  expect(mocks.close).not.toHaveBeenCalled();

  const replacement = session("one", { sessionId: "replacement", createdAt: "2026-11-15T00:00:03Z" });
  mocks.sessions.mockImplementation(async (id: string) => id === "one" ? [session(id), replacement] : [session(id)]);
  const activeAtBackendClose: Array<string | null> = [];
  mocks.close.mockImplementation(async () => {
    activeAtBackendClose.push(useTerminalStore.getState().activeSessionId);
    return session("one", { status: "disconnected" });
  });
  const activeAtSftpRemoval: Array<string | null> = [];
  const unsubscribe = useSftpStore.subscribe((state, previous) => {
    if (previous.tabs["one-session"] && !state.tabs["one-session"]) {
      activeAtSftpRemoval.push(useTerminalStore.getState().activeSessionId);
    }
  });
  try {
    act(() => workspaceProps().onReconnect("one-session"));
    await waitFor(() => expect(mocks.connect).toHaveBeenCalledTimes(2));
    expect(mocks.close).not.toHaveBeenCalled();
    expect(useSftpStore.getState().tabs["one-session"]).toBe(originalSftp);
    await act(async () => retry.resolve(replacement));
    await waitFor(() => expect(mocks.close).toHaveBeenCalledExactlyOnceWith({ workspaceId: "one", sessionId: "one-session" }));
    expect(activeAtBackendClose).toEqual(["replacement"]);
    expect(activeAtSftpRemoval).toEqual(["replacement"]);
    expect(useTerminalStore.getState().dismissedSessionIds).toContain("one-session");
    expect(useSftpStore.getState().tabs["one-session"]).toBeUndefined();
    await waitFor(() => expect(workspaceProps().activeSessionId).toBe("replacement"));
    expect(page.client.getQueryData<SshSessionSummary[]>(["ssh-sessions", "one"]))
      .toEqual(expect.arrayContaining([replacement]));
  } finally {
    unsubscribe();
  }
});

it("ignores an old workspace close completion and preserves the new workspace events", async () => {
  const pending = deferred<SshSessionSummary>();
  mocks.close.mockReturnValueOnce(pending.promise);
  const page = mountPage();
  act(() => workspaceProps().onCloseSession("one-session"));
  fireEvent.click(await screen.findByRole("button", { name: "Close Session" }));
  await waitFor(() => expect(mocks.close).toHaveBeenCalledWith({ workspaceId: "one", sessionId: "one-session" }));
  page.update({ workspaceId: "two" });
  await waitFor(() => expect(workspaceProps().activeSessionId).toBe("two-session"));
  act(() => useTerminalStore.getState().appendTerminalEvents([event("two")]));
  await act(async () => pending.resolve(session("one", { status: "disconnected" })));
  expect(useTerminalStore.getState().terminalEvents).toEqual([event("two")]);
  expect(workspaceProps().activeSessionId).toBe("two-session");
});

it("freezes hidden output while retaining the channel and resumes events and polling on activation", async () => {
  const page = mountPage();
  await waitFor(() => expect(mocks.channel).toHaveBeenCalledOnce());
  const eventsBeforeHide = workspaceProps().events;
  page.update({ active: false });
  const hiddenRenderCount = mocks.workspace.mock.calls.length;
  const receive = mocks.channel.mock.lastCall![0] as (payload: SshTerminalDataPayload) => void;
  act(() => receive({ sessionId: "one-session", data: "while hidden" }));
  await waitFor(() => expect(useTerminalStore.getState().terminalEvents).toHaveLength(2));
  expect(mocks.workspace).toHaveBeenCalledTimes(hiddenRenderCount);
  expect(workspaceProps().events).toBe(eventsBeforeHide);
  expect(mocks.disposeChannel).not.toHaveBeenCalled();
  expect(page.client.getQueryCache().find({ queryKey: ["ssh-sessions", "one"] })?.options)
    .toMatchObject({ enabled: false, refetchInterval: false });
  expect(page.client.getQueryCache().find({ queryKey: ["ssh-connections", "one"] })?.options)
    .toMatchObject({ enabled: false });
  page.update({ active: true });
  expect(workspaceProps().events.map(({ data }) => data)).toEqual(["one output", "while hidden"]);
  expect(page.client.getQueryCache().find({ queryKey: ["ssh-sessions", "one"] })?.options)
    .toMatchObject({ enabled: true, refetchInterval: 2_000 });
  expect(mocks.channel).toHaveBeenCalledOnce();
});

it("keeps creation order and the active tab when polling returns a different update order", async () => {
  const page = mountPage();
  const first = session("one");
  const second = session("one", { sessionId: "second", createdAt: "2026-11-15T00:00:02Z" });
  act(() => {
    page.client.setQueryData(["ssh-sessions", "one"], [second, first]);
    useTerminalStore.getState().setActiveSessionId("second");
  });
  await waitFor(() => expect(workspaceProps().sessions.map(({ session: item }) => item.sessionId))
    .toEqual(["one-session", "second"]));
  act(() => page.client.setQueryData(["ssh-sessions", "one"], [
    { ...first, updatedAt: "2026-11-15T00:00:08Z" },
    { ...second, updatedAt: "2026-11-15T00:00:07Z" },
  ]));
  await waitFor(() => expect(workspaceProps().sessions[0].session.updatedAt).toBe("2026-11-15T00:00:08Z"));
  expect(workspaceProps().sessions.map(({ session: item }) => item.sessionId)).toEqual(["one-session", "second"]);
  expect(workspaceProps().activeSessionId).toBe("second");
});

it("keeps sidebar callbacks stable and clears the injected node on hide, workspace switch, and unmount", async () => {
  const sidebar = vi.fn<(node: ReactNode | null) => void>();
  const showSaved = vi.fn();
  const page = mountPage({ onShellSidebarChange: sidebar, onShowSaved: showSaved });
  await waitFor(() => expect(sidebar).toHaveBeenCalledOnce());
  const tree = sidebar.mock.calls[0][0] as ReactElement<ComponentProps<typeof SshConnectionTree>>;
  act(() => useTerminalStore.getState().appendTerminalEvents([event("one", "more output")]));
  act(() => tree.props.onNewConnection());
  expect(await screen.findByRole("dialog")).toBeVisible();
  expect(showSaved).toHaveBeenCalledOnce();
  expect(sidebar).toHaveBeenCalledOnce();
  page.update({ active: false, sidebarActive: false });
  expect(sidebar).toHaveBeenLastCalledWith(null);
  page.update({ active: true, sidebarActive: true });
  expect(sidebar).toHaveBeenLastCalledWith(tree);
  page.update({ workspaceId: "two" });
  const calls = sidebar.mock.calls.map(([node]) => node);
  expect(calls[calls.length - 2]).toBeNull();
  expect((calls[calls.length - 1] as ReactElement<ComponentProps<typeof SshConnectionTree>>).props.workspaceId).toBe("two");
  page.unmount();
  expect(sidebar).toHaveBeenLastCalledWith(null);
});
