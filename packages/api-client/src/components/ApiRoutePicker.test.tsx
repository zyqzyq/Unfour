// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { getApiRoutes, listSshConnections, listTemporaryRouteHandles, setApiEnvironmentRoute, type SshConnection } from "@unfour/command-client";
import { useState } from "react";
import { useApiRoutes } from "../hooks/useApiRoutes";
import { createNewRequestTab, emptyApiTabsState } from "../model/request-tabs";
import type { ApiRoute, ApiRouteEdit } from "@unfour/command-client";
import { ApiRoutePicker } from "./ApiRoutePicker";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(),
  getApiRoutes: vi.fn(), listSshConnections: vi.fn(), setApiEnvironmentRoute: vi.fn(),
  listTemporaryRouteHandles: vi.fn().mockResolvedValue([]),
}));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const tab = createNewRequestTab(emptyApiTabsState("w"), "new:1").tabs[0];
const sshRoute: ApiRoute = { mode: "ssh", sshConnectionId: "ssh-1" };
function TestBar({ onApplied }: { onApplied?: (route: ApiRouteEdit | undefined) => void } = {}) {
  const [draft, setDraft] = useState<ApiRouteEdit>();
  const [handle, setHandle] = useState<string>();
  const routing = useApiRoutes("w", { ...tab, draft: { ...tab.draft, route: draft, temporaryHandle: handle } }, "env-1", "success");
  return <ApiRoutePicker routing={routing} draft={draft} onChange={(route, live) => { onApplied?.(route); setDraft(route); setHandle(live); }} disabled={false} environmentName="QA" />;
}

it("offers a bastion session for this request only", async () => {
  vi.mocked(getApiRoutes).mockResolvedValue({ environmentId: "env-1", request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" } });
  vi.mocked(listSshConnections).mockResolvedValue([]);
  vi.mocked(listTemporaryRouteHandles).mockResolvedValue(["session-12345678"]);
  const client = new QueryClient();
  const onApplied = vi.fn();
  render(<QueryClientProvider client={client}><TestBar onApplied={onApplied} /></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: /Network route · Environment · Direct/ }));
  const request = screen.getByLabelText("This request");
  await within(request).findByRole("option", { name: "Bastion session · 12345678" });
  expect(within(screen.getByLabelText("Default for QA")).queryByRole("option", { name: /Bastion session/ })).toBeNull();
  fireEvent.change(request, { target: { value: "direct" } });
  fireEvent.change(request, { target: { value: "live:session-12345678" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(await screen.findByRole("button", { name: "Network route · Bastion session · 12345678" })).toBeVisible();
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
