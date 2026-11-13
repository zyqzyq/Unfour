// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { getApiRoutes, listSshConnections, setApiEnvironmentRoute, type SshConnection } from "@unfour/command-client";
import { useState } from "react";
import { useApiRoutes } from "../hooks/useApiRoutes";
import { createNewRequestTab, emptyApiTabsState } from "../model/request-tabs";
import type { ApiRoute, ApiRouteEdit } from "@unfour/command-client";
import { ApiRoutePicker } from "./ApiRoutePicker";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(),
  getApiRoutes: vi.fn(), listSshConnections: vi.fn(), setApiEnvironmentRoute: vi.fn(),
}));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const tab = createNewRequestTab(emptyApiTabsState("w"), "new:1").tabs[0];
const sshRoute: ApiRoute = { mode: "ssh", sshConnectionId: "ssh-1" };
function TestBar() {
  const [draft, setDraft] = useState<ApiRouteEdit>();
  const routing = useApiRoutes("w", { ...tab, draft: { ...tab.draft, route: draft } }, "env-1", "success");
  return <ApiRoutePicker routing={routing} draft={draft} onChange={setDraft} disabled={false} environmentName="QA" />;
}

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
