// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { FlowPage } from "../../../../packages/flow/src/FlowPage";
import { useCloudSyncQueryRefresh } from "../../src/features/cloud-sync/useCloudSyncQueryRefresh";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  subscribeCloudSyncWorkspaceChanges: vi.fn(), listFlows: vi.fn(), listFlowRuns: vi.fn(),
  listSavedApiRequests: vi.fn(), listSshTasks: vi.fn(), listDatabaseConnections: vi.fn(),
  listSshConnections: vi.fn(), listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(),
}));
const flow: commands.FlowDefinition = {
  id: "flow-1", workspaceId: "ws", name: "Release", revision: 3, inputs: [],
  steps: [{ id: "wait", name: "Wait", kind: "wait", durationMs: 10, timeoutMs: 1000, next: null }],
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  vi.mocked(commands.listFlows).mockResolvedValue([flow]);
  for (const command of [commands.listFlowRuns, commands.listSavedApiRequests, commands.listSshTasks,
    commands.listDatabaseConnections, commands.listSshConnections,
    commands.listWorkspaceEnvironments, commands.listWorkspaceVariables]) vi.mocked(command).mockResolvedValue([]);
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
function Shell() {
  useCloudSyncQueryRefresh();
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return <>{sidebar}<FlowPage workspaceId="ws" onSidebarContentChange={setSidebar} /></>;
}
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><I18nProvider initialLocale="en"><Shell /></I18nProvider></QueryClientProvider>);
}
function notify() {
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0]("ws"));
}

it("shows cloud-synced definitions without reopening an initially empty Flow page", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([]);
  mount();
  await screen.findByRole("heading", { name: "Create your first Flow" });
  vi.mocked(commands.listFlows).mockResolvedValue([flow]);
  notify();
  fireEvent.click(await screen.findByText("Release"));
  expect(screen.getByLabelText("Flow name")).toHaveValue("Release");
});

it("refreshes synced Flow names and deletions while preserving an unsaved editor draft", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Flow name"), { target: { value: "Unsaved draft" } });
  vi.mocked(commands.listFlows).mockResolvedValue([{ ...flow, name: "Remote release", revision: 4 }]);
  notify();
  await screen.findByText("Remote release");
  expect(screen.getByLabelText("Flow name")).toHaveValue("Unsaved draft");
  vi.mocked(commands.listFlows).mockResolvedValue([]);
  notify();
  await waitFor(() => expect(screen.queryByText("Remote release")).not.toBeInTheDocument());
  expect(screen.getByLabelText("Flow name")).toHaveValue("Unsaved draft");
});
