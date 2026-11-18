// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { getApiRoutes, listSshConnections, listTemporaryRoutes, setApiEnvironmentRoute, type SshConnection } from "@unfour/command-client";
import { useState } from "react";
import { useApiRoutes } from "../hooks/useApiRoutes";
import { createNewRequestTab, emptyApiTabsState } from "../model/request-tabs";
import type { ApiRoute, ApiRouteEdit, TemporaryApiExecutionMode } from "@unfour/command-client";
import { ApiRoutePicker } from "./ApiRoutePicker";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(),
  getApiRoutes: vi.fn(), listSshConnections: vi.fn(), setApiEnvironmentRoute: vi.fn(),
  listTemporaryRoutes: vi.fn().mockResolvedValue([]),
}));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const tab = createNewRequestTab(emptyApiTabsState("w"), "new:1").tabs[0];
const sshRoute: ApiRoute = { mode: "ssh", sshConnectionId: "ssh-1" };
function TestBar({ onApplied }: { onApplied?: (route: ApiRouteEdit | undefined) => void } = {}) {
  const [draft, setDraft] = useState<ApiRouteEdit>();
  const [handle, setHandle] = useState<string>();
  const [mode, setMode] = useState<TemporaryApiExecutionMode>();
  const routing = useApiRoutes("w", { ...tab, draft: { ...tab.draft, route: draft, temporaryHandle: handle } }, "env-1", "success");
  return <ApiRoutePicker routing={routing} draft={draft} temporaryExecutionMode={mode} onChange={(route, live, executionMode) => { onApplied?.(route); setDraft(route); setHandle(live); setMode(executionMode); }} disabled={false} environmentName="QA" />;
}

it("offers terminal curl explicitly without changing the stored environment route", async () => {
  vi.mocked(getApiRoutes).mockResolvedValue({ environmentId: "env-1", request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" } });
  vi.mocked(listSshConnections).mockResolvedValue([]);
  vi.mocked(listTemporaryRoutes).mockResolvedValue([{ handle: "live-node", displayName: "Target node", endpoint: "fixture.invalid:22" }]);
  const client = new QueryClient();
  render(<QueryClientProvider client={client}><TestBar /></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: /Network route · Environment · Direct/ }));
  const request = screen.getByLabelText("This request");
  await within(request).findByRole("option", { name: "Target node · fixture.invalid:22" });
  expect(screen.queryByText("Add a connection in SSH to route requests through it.")).not.toBeInTheDocument();
  fireEvent.change(request, { target: { value: "live:live-node" } });
  expect(screen.getByLabelText("Execution mode")).toHaveValue("tcp");
  fireEvent.change(screen.getByLabelText("Execution mode"), { target: { value: "terminalCurl" } });
  expect(screen.getByText(/The selected host sends the request with curl/)).toBeInTheDocument();
  expect(screen.queryByText("Add a connection in SSH to route requests through it.")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(await screen.findByRole("button", { name: /Network route · Terminal curl · Target node/ })).toBeInTheDocument();
  expect(setApiEnvironmentRoute).not.toHaveBeenCalled();
});

it("shows connection setup help only after both saved and live hosts are confirmed empty", async () => {
  vi.mocked(getApiRoutes).mockResolvedValue({ environmentId: "env-1", request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" } });
  vi.mocked(listSshConnections).mockResolvedValue([]);
  let resolveSessions!: (routes: []) => void;
  vi.mocked(listTemporaryRoutes).mockReturnValue(new Promise((resolve) => { resolveSessions = resolve; }));
  const client = new QueryClient();
  render(<QueryClientProvider client={client}><TestBar /></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: /Network route · Environment · Direct/ }));
  expect(screen.queryByText("Add a connection in SSH to route requests through it.")).not.toBeInTheDocument();
  resolveSessions([]);
  expect(await screen.findByText("Add a connection in SSH to route requests through it.")).toBeVisible();
});

it("offers a bastion session for this request only", async () => {
  vi.mocked(getApiRoutes).mockResolvedValue({ environmentId: "env-1", request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" } });
  vi.mocked(listSshConnections).mockResolvedValue([]);
  vi.mocked(listTemporaryRoutes).mockResolvedValue([{ handle: "session-12345678", displayName: "Production node", endpoint: "10.0.0.8:22" }]);
  const client = new QueryClient();
  const onApplied = vi.fn();
  render(<QueryClientProvider client={client}><TestBar onApplied={onApplied} /></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: /Network route · Environment · Direct/ }));
  const request = screen.getByLabelText("This request");
  await within(request).findByRole("option", { name: "Production node · 10.0.0.8:22" });
  expect(within(screen.getByLabelText("Default for QA")).queryByRole("option", { name: /Bastion session/ })).toBeNull();
  fireEvent.change(request, { target: { value: "direct" } });
  fireEvent.change(request, { target: { value: "live:session-12345678" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(await screen.findByRole("button", { name: "Network route · Production node · 10.0.0.8:22" })).toBeVisible();
  expect(onApplied).toHaveBeenCalledWith(undefined);
  expect(setApiEnvironmentRoute).not.toHaveBeenCalled();
});

it("shows inherited SSH before Send and keeps the request override separate from the environment default", async () => {
  vi.mocked(getApiRoutes).mockResolvedValue({
    environmentId: "env-1", request: { mode: "inherit" }, environment: sshRoute, effective: sshRoute,
  });
  vi.mocked(listSshConnections).mockResolvedValue([{ id: "ssh-1", name: "Build host" } as SshConnection]);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestBar /></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: "Network route · Environment · SSH · Build host" }));
  fireEvent.change(screen.getByLabelText("This request"), { target: { value: "direct" } });
  expect(screen.getByLabelText("Default for QA")).toHaveValue("ssh:ssh-1");
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(await screen.findByRole("button", { name: "Network route · Direct" })).toBeVisible();
  expect(setApiEnvironmentRoute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Network route · Direct" }));
  fireEvent.change(screen.getByLabelText("Default for QA"), { target: { value: "direct" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  await waitFor(() => expect(setApiEnvironmentRoute).toHaveBeenCalledWith("w", "env-1", { expected: sshRoute, route: { mode: "direct" } }));
});
