// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { closeSshSession, connectSshSession, deleteSshConnection, listSshConnections, listSshSessions, saveSshConnection, type SshConnection, type SshSessionSummary } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { I18nProvider } from "@unfour/ui";
import { useTerminalStore } from "../model/terminal-state";
import { SshConnectionTree } from "./SshConnectionTree";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listSshConnections: vi.fn(), listSshSessions: vi.fn(), connectSshSession: vi.fn(),
  closeSshSession: vi.fn(), deleteSshConnection: vi.fn(), saveSshConnection: vi.fn(),
}));
const production: SshConnection = {
  id: "prod", workspaceId: "ws", name: "Production", host: "db.example.com", username: "deploy", port: 2222,
  authKind: "password", credentialRef: "secret-reference", keyPath: null, createdAt: "now", updatedAt: "now",
  deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
};
const local = { ...production, id: "local", name: "Local", host: "127.0.0.1", username: "tester", port: 22 };
const session: SshSessionSummary = {
  sessionId: "live", workspaceId: "ws", connectionId: "prod", status: "connected", reconnectAttempt: 0,
  authKind: "password", host: production.host, username: "deploy", cols: 120, rows: 32, createdAt: "now", updatedAt: "now",
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listSshConnections).mockImplementation(async (workspaceId) => workspaceId === "ws" ? [production, local] : [{ ...local, id: "next", workspaceId, name: "Next workspace" }]);
  vi.mocked(listSshSessions).mockResolvedValue([session]);
  vi.mocked(connectSshSession).mockResolvedValue(session);
  useWorkspaceStore.setState({ selectedSshConnectionId: "prod" });
  useTerminalStore.setState({ activeSessionId: "live", terminalInput: "unfinished", splitMode: "vertical", terminalEvents: [{ sessionId: "live", kind: "output", data: "existing output", createdAt: "now" }] });
});
afterEach(cleanup);
function setup(locale: "en" | "zh-CN" = "en") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity }, mutations: { retry: false } } });
  const onOpenTerminal = vi.fn();
  const onOpenTasks = vi.fn();
  const onEditConnection = vi.fn();
  const node = (workspaceId: string) => <QueryClientProvider client={client}><I18nProvider initialLocale={locale}>
    <SshConnectionTree workspaceId={workspaceId} onOpenTerminal={onOpenTerminal} onOpenTasks={onOpenTasks} onEditConnection={onEditConnection} />
  </I18nProvider></QueryClientProvider>;
  const view = render(node("ws"));
  return { ...view, node, client, onOpenTerminal, onOpenTasks, onEditConnection };
}
function search(value: string) {
  fireEvent.change(screen.getByRole("textbox", { name: "Search SSH connections" }), { target: { value } });
}

describe("SSH connection local search", () => {
  it.each(["  PrOdUcTion ", "DB.EXAMPLE", "deploy", "2222"])("matches existing connection fields: %s", async (query) => {
    setup();
    await screen.findByRole("button", { name: "Production" });
    search(query);
    expect(screen.getByRole("button", { name: "Production" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Local" })).not.toBeInTheDocument();
  });

  it("clears no results without changing selection, sessions, cached data or issuing commands", async () => {
    const { client, onOpenTasks } = setup();
    await screen.findByRole("button", { name: "Production" });
    const terminalState = useTerminalStore.getState();
    const data = client.getQueryData(["ssh-connections", "ws"]);
    search("secret-reference");
    expect(screen.getByText("No matching SSH connections")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Tasks" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Tasks" }));
    expect(onOpenTasks).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Local" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Production" }).closest("[role=treeitem]")).toHaveAttribute("aria-selected", "true");
    expect(useWorkspaceStore.getState().selectedSshConnectionId).toBe("prod");
    expect(useTerminalStore.getState()).toBe(terminalState);
    expect(client.getQueryData(["ssh-connections", "ws"])).toBe(data);
    expect(listSshConnections).toHaveBeenCalledTimes(1);
    expect(listSshSessions).toHaveBeenCalledTimes(1);
    for (const fn of [connectSshSession, closeSshSession, deleteSshConnection, saveSshConnection]) expect(fn).not.toHaveBeenCalled();
  });

  it("resets search and never renders previous Workspace results while loading the next", async () => {
    const { rerender, node } = setup();
    await screen.findByRole("button", { name: "Production" });
    search("Production");
    rerender(node("ws-2"));
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.queryByRole("button", { name: "Production" })).not.toBeInTheDocument();
    await screen.findByRole("button", { name: "Next workspace" });
    search("Production");
    expect(screen.getByText("No matching SSH connections")).toBeInTheDocument();
    rerender(node("ws"));
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(await screen.findByRole("button", { name: "Local" })).toBeInTheDocument();
  });

  it("retains selection, connection activation and context menu actions for results", async () => {
    const { onOpenTerminal, onEditConnection } = setup();
    await screen.findByRole("button", { name: "Local" });
    search("127.0.0.1");
    fireEvent.click(screen.getByRole("button", { name: "Local" }));
    expect(useWorkspaceStore.getState().selectedSshConnectionId).toBe("local");
    expect(onOpenTerminal).toHaveBeenCalledOnce();
    expect(connectSshSession).not.toHaveBeenCalled();
    fireEvent.contextMenu(screen.getByRole("button", { name: "Local" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Edit Connection" }));
    expect(onEditConnection).toHaveBeenCalledExactlyOnceWith(local);
    fireEvent.doubleClick(screen.getByRole("button", { name: "Local" }));
    await waitFor(() => expect(connectSshSession).toHaveBeenCalledExactlyOnceWith({ workspaceId: "ws", connectionId: "local", cols: 120, rows: 32 }));
  });

  it("supports Chinese search and no-result labels", async () => {
    setup("zh-CN");
    await screen.findByRole("button", { name: "Production" });
    const input = screen.getByRole("textbox", { name: "搜索 SSH 连接" });
    fireEvent.change(input, { target: { value: "不存在" } });
    expect(screen.getByText("没有匹配的 SSH 连接")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清空搜索" }));
    expect(input).toHaveValue("");
  });
});
