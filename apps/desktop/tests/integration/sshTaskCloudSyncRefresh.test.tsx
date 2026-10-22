// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { useCloudSyncQueryRefresh } from "../../src/features/cloud-sync/useCloudSyncQueryRefresh";
import { SshTasksPage } from "../../../../packages/ssh-terminal/src/components/SshTasksPage";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  subscribeCloudSyncWorkspaceChanges: vi.fn(), listSshTasks: vi.fn(), getSshTask: vi.fn(),
  listSshTaskRuns: vi.fn(), runSshTask: vi.fn(), saveSshTask: vi.fn(),
}));
const task: commands.SshTask = {
  id: "task", workspaceId: "ws", name: "Release task", description: "", sortOrder: 0,
  createdAt: "now", updatedAt: "now", deletedAt: null,
};
const detail: commands.SshTaskDetail = {
  task, localBinding: null, detectedInputs: [], steps: [{
    id: "step", workspaceId: "ws", taskId: "task", name: "Command", stepType: "command", position: 0,
    enabled: true, configVersion: 1, createdAt: "now", updatedAt: "now", deletedAt: null,
    configJson: { command: "echo original", workingDirectory: "", timeoutSeconds: 60, continueOnError: false },
  }],
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  vi.mocked(commands.listSshTasks).mockResolvedValue([]);
  vi.mocked(commands.listSshTaskRuns).mockResolvedValue([]);
  vi.mocked(commands.getSshTask).mockResolvedValue(detail);
});
afterEach(cleanup);
const openConnections = () => {};
function Shell() {
  useCloudSyncQueryRefresh();
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return <><aside data-testid="task-list">{sidebar}</aside>
    <SshTasksPage workspaceId="ws" connections={[]} onOpenConnections={openConnections} onShellSidebarChange={setSidebar} />
  </>;
}
function notify() {
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0]("ws"));
}

it("receives task creation, step updates and deletion while retaining the current editor draft", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  render(<QueryClientProvider client={client}><I18nProvider initialLocale="en"><Shell /></I18nProvider></QueryClientProvider>);
  await waitFor(() => expect(client.getQueryData(["ssh-tasks", "ws"])).toEqual([]));
  vi.mocked(commands.listSshTasks).mockResolvedValue([task]);
  notify();
  fireEvent.click(await within(screen.getByTestId("task-list")).findByText("Release task"));
  const name = await screen.findByDisplayValue("Release task");
  await screen.findByDisplayValue("echo original");
  fireEvent.change(name, { target: { value: "Unsaved task" } });
  const remote: commands.SshTaskDetail = {
    ...detail, task: { ...task, name: "Remote task", updatedAt: "later" },
    steps: [{ ...detail.steps[0], configJson: { command: "echo remote", workingDirectory: "", timeoutSeconds: 60, continueOnError: false } }],
  };
  vi.mocked(commands.listSshTasks).mockResolvedValue([remote.task]);
  vi.mocked(commands.getSshTask).mockResolvedValue(remote);
  notify();
  await within(screen.getByTestId("task-list")).findByText("Remote task");
  await waitFor(() => expect(client.getQueryData<commands.SshTaskDetail>(["ssh-task", "ws", "task"])?.steps[0].configJson).toEqual(remote.steps[0].configJson));
  expect(screen.getByDisplayValue("Unsaved task")).toBeInTheDocument();
  expect(screen.getByDisplayValue("echo original")).toBeInTheDocument();
  vi.mocked(commands.listSshTasks).mockResolvedValue([]);
  vi.mocked(commands.getSshTask).mockRejectedValue(new Error("task not found"));
  notify();
  await waitFor(() => expect(within(screen.getByTestId("task-list")).queryByText("Remote task")).not.toBeInTheDocument());
  expect(screen.getByDisplayValue("Unsaved task")).toBeInTheDocument();
  expect(commands.runSshTask).not.toHaveBeenCalled();
  expect(commands.saveSshTask).not.toHaveBeenCalled();
});
