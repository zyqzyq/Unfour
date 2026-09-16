// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nProvider } from "@unfour/ui";
import * as commands from "@unfour/command-client";
import { FlowPage } from "./FlowPage";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@unfour/command-client")>()),
  listFlows: vi.fn(),
  listFlowRuns: vi.fn(),
  getFlowRun: vi.fn(),
  saveFlow: vi.fn(),
  runFlow: vi.fn(),
  cancelFlowRun: vi.fn(),
  listSavedApiRequests: vi.fn(),
  listSshTasks: vi.fn(),
  listDatabaseConnections: vi.fn(),
  listSshConnections: vi.fn(),
  listWorkspaceEnvironments: vi.fn(),
}));
const flow: commands.FlowDefinition = {
  id: "flow-1",
  workspaceId: "ws",
  name: "Release",
  revision: 3,
  inputs: ["version"],
  steps: [
    {
      id: "wait",
      name: "Wait",
      kind: "wait",
      durationMs: 10,
      timeoutMs: 1000,
      next: null,
    },
  ],
};
const run: commands.FlowRun = {
  id: "run-1",
  workspaceId: "ws",
  flowId: flow.id,
  definition: flow,
  context: {
    workspaceId: "ws",
    flowId: flow.id,
    environmentId: null,
    inputs: { version: 42 },
    initiator: "human",
    confirmEffects: true,
  },
  resources: {},
  status: "running",
  error: null,
  startedAt: "2026-09-14T00:00:00Z",
  finishedAt: null,
  steps: [
    {
      stepId: "wait",
      status: "running",
      attempts: [
        {
          number: 1,
          input: { version: 42 },
          output: null,
          error: null,
          durationMs: 0,
        },
      ],
      durationMs: 0,
      error: null,
    },
  ],
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(commands.listFlows).mockResolvedValue([structuredClone(flow)]);
  vi.mocked(commands.listFlowRuns).mockResolvedValue([structuredClone(run)]);
  vi.mocked(commands.getFlowRun).mockResolvedValue(structuredClone(run));
  vi.mocked(commands.runFlow).mockResolvedValue(run);
  vi.mocked(commands.saveFlow).mockImplementation(async (input) => ({
    ...input,
    id: input.id || "new",
    revision: input.revision + 1,
  }));
  for (const command of [
    commands.listSavedApiRequests,
    commands.listSshTasks,
    commands.listDatabaseConnections,
    commands.listSshConnections,
    commands.listWorkspaceEnvironments,
  ])
    vi.mocked(command).mockResolvedValue([]);
});
afterEach(cleanup);
function Harness() {
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return (
    <>
      {sidebar}
      <FlowPage workspaceId="ws" onSidebarContentChange={setSidebar} />
    </>
  );
}
function mount() {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <I18nProvider initialLocale="en">
        <Harness />
      </I18nProvider>
    </QueryClientProvider>,
  );
}

it("sends explicit context only after run confirmation and supports cancellation", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), {
    target: { value: '{"version":42}' },
  });
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  expect(commands.runFlow).not.toHaveBeenCalled();
  const dialog = await screen.findByRole("dialog");
  fireEvent.click(dialog.querySelector("button:last-child")!);
  await waitFor(() =>
    expect(commands.runFlow).toHaveBeenCalledWith({
      workspaceId: "ws",
      flowId: "flow-1",
      environmentId: null,
      inputs: { version: 42 },
      secretInputNames: [],
      initiator: "human",
      confirmEffects: true,
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Cancel run" }));
  await waitFor(() =>
    expect(commands.cancelFlowRun).toHaveBeenCalledWith("ws", "run-1"),
  );
  expect(screen.getByText("r3", { selector: "code" })).toBeInTheDocument();
});

it("blocks invalid JSON instead of saving the last valid value", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Add step"), {
    target: { value: "poll" },
  });
  fireEvent.change(screen.getByLabelText("Arguments (JSON)"), {
    target: { value: '{"url":' },
  });
  fireEvent.change(screen.getByLabelText("Predicate (left / op / right)"), {
    target: { value: '{"left":true,"op":"eq","right":true}' },
  });
  expect(
    screen.getByRole("button", { name: "Save", exact: true }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Run", exact: true }),
  ).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Arguments (JSON)"), {
    target: { value: "{}" },
  });
  expect(
    screen.getByRole("button", { name: "Save", exact: true }),
  ).toBeEnabled();
});

it("requires a deliberate discard when switching a dirty definition", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Flow name"), {
    target: { value: "Changed" },
  });
  fireEvent.click(screen.getByRole("button", { name: "New Flow" }));
  expect(await screen.findByRole("dialog")).toHaveTextContent(
    "Discard unsaved changes",
  );
  expect(screen.getByLabelText("Flow name")).toHaveValue("Changed");
});
