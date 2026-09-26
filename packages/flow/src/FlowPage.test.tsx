// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
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
  listWorkspaceVariables: vi.fn(),
  getSshTask: vi.fn(),
}));
const flow: commands.FlowDefinition = {
  id: "flow-1",
  workspaceId: "ws",
  name: "Release",
  revision: 3,
  inputs: [{ name: "version", type: "number", required: true, secret: false }],
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
const summary = ({ id, flowId, status, startedAt, finishedAt }: commands.FlowRun): commands.FlowRunSummary => ({ id, flowId, status, startedAt, finishedAt });
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
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(commands.listFlows).mockResolvedValue([structuredClone(flow)]);
  vi.mocked(commands.listFlowRuns).mockResolvedValue([summary(run)]);
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
    commands.listWorkspaceVariables,
  ])
    vi.mocked(command).mockResolvedValue([]);
});
afterEach(cleanup);

it("clears stale run-field validity after changing its schema type", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([{ ...flow, inputs: [{ name: "payload", type: "json", required: false, secret: false }] }]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("payload"), { target: { value: "{" } });
  expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  await closeRun();
  fireEvent.change(screen.getByLabelText("Type"), { target: { value: "string" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());
});

it("requires discarding an invalid input-default draft before switching", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([
    { ...flow, inputs: [{ name: "payload", type: "json", required: false, secret: false, default: {} }] },
    { ...flow, id: "other", name: "Other" },
  ]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Default (optional)"), { target: { value: "{" } });
  fireEvent.click(screen.getByText("Other"));
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(screen.getByLabelText("Flow name")).toHaveValue("Release");
});
function openRun() {
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
}
async function closeRun() {
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run", exact: true })).toHaveFocus());
}
async function selectHistory() {
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  fireEvent.click(await screen.findByRole("button", { name: /2026-09-14T00:00:00Z/ }));
}
function Harness({ workspaceId = "ws" }: { workspaceId?: string }) {
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return (
    <>
      {sidebar}
      <FlowPage workspaceId={workspaceId} onSidebarContentChange={setSidebar} />
    </>
  );
}
function mount(locale: "en" | "zh-CN" = "en") {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <I18nProvider initialLocale={locale}>
        <Harness />
      </I18nProvider>
    </QueryClientProvider>,
  );
}

it("sends explicit context only after run confirmation and supports cancellation", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), {
    target: { value: '{"version":42}' },
  });
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
    target: { value: "waitUntil" },
  });
  fireEvent.change(screen.getByLabelText("Arguments (JSON)"), {
    target: { value: '{"url":' },
  });
  fireEvent.change(screen.getByLabelText("Success when (left / op / right)"), {
    target: { value: '{"left":true,"op":"eq","right":true}' },
  });
  expect(
    screen.getByRole("button", { name: "Save" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Run" }),
  ).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Arguments (JSON)"), {
    target: { value: "{}" },
  });
  expect(
    screen.getByRole("button", { name: "Save" }),
  ).toBeEnabled();
});

it("resets inputs, secrets and explicit environment on selection and discard to a new Flow", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([flow, { ...flow, id: "other", name: "Other", inputs: [] }]);
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ id: "prod", name: "Production" }] as Awaited<ReturnType<typeof commands.listWorkspaceEnvironments>>);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), { target: { value: '{"version":42}' } });
  fireEvent.change(screen.getByLabelText("Secret input names (comma separated)"), { target: { value: "version" } });
  fireEvent.change(screen.getByLabelText("Environment"), { target: { value: "prod" } });
  await closeRun();
  fireEvent.click(screen.getByText("Other"));
  openRun();
  expect(screen.getByLabelText("Run inputs (JSON object)")).toHaveValue("{}");
  expect(screen.getByLabelText("Secret input names (comma separated)")).toHaveValue("");
  expect(screen.getByLabelText("Environment")).toHaveValue("");
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), { target: { value: '{"private":"x"}' } });
  await closeRun();
  fireEvent.change(screen.getByLabelText("Flow name"), { target: { value: "Unsaved" } });
  fireEvent.click(screen.getByRole("button", { name: "New Flow" }));
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Discard" }));
  vi.mocked(commands.saveFlow).mockResolvedValue({ ...flow, id: "new", inputs: [] });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());
  openRun();
  expect(screen.getByLabelText("Run inputs (JSON object)")).toHaveValue("{}");
});

it("remounts all context when workspace changes", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = (workspaceId: string) => <QueryClientProvider client={client}><I18nProvider initialLocale="en"><Harness workspaceId={workspaceId} /></I18nProvider></QueryClientProvider>;
  const mounted = render(view("ws"));
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), { target: { value: '{"version":42}' } });
  mounted.rerender(view("next"));
  expect(screen.queryByLabelText("Flow name")).not.toBeInTheDocument();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  expect(screen.getByLabelText("Run inputs (JSON object)")).toHaveValue("{}");
});

it("generates typed fields, checks required inputs and masks confirmation context", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([{ ...flow, inputs: [
    ...flow.inputs,
    { name: "enabled", type: "boolean", required: true, secret: false, default: false },
    { name: "credential", type: "string", required: true, secret: true },
  ] }]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("version", { exact: true }), { target: { value: "42" } });
  const secret = screen.getByLabelText("credential", { exact: true });
  expect(secret).toHaveAttribute("type", "password");
  fireEvent.change(secret, { target: { value: "never-show" } });
  const dialog = await screen.findByRole("dialog");
  expect(dialog).toHaveTextContent("Release");
  expect(dialog).toHaveTextContent("ws");
  expect(dialog).toHaveTextContent("Workspace variables only");
  expect(dialog).toHaveTextContent("42");
  expect(dialog).toHaveTextContent("false");
  expect(within(dialog).getByLabelText("Input preview (masked)")).not.toHaveTextContent("never-show");
});

it("blocks running broken resources while preserving Save", async () => {
  const broken: commands.FlowDefinition = { ...flow, inputs: [], steps: [{ id: "api", name: "Fetch", kind: "action", timeoutMs: 1000, next: null, action: { capability: "api", resourceId: "deleted", connectionId: null, arguments: {} } }] };
  vi.mocked(commands.listFlows).mockResolvedValue([broken]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(await screen.findByText(/Referenced resource is missing/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

it("blocks Save and Run for blank or duplicate input definitions", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.click(screen.getByRole("button", { name: "Add input" }));
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  fireEvent.change(screen.getAllByLabelText("Input name")[1], { target: { value: "version" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  fireEvent.change(screen.getAllByLabelText("Input name")[1], { target: { value: "other" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

it("cannot switch definitions while a save is pending", async () => {
  let finishSave!: (flow: commands.FlowDefinition) => void;
  vi.mocked(commands.listFlows).mockResolvedValue([flow, { ...flow, id: "other", name: "Other" }]);
  vi.mocked(commands.saveFlow).mockImplementation(() => new Promise((resolve) => { finishSave = resolve; }));
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  fireEvent.click(screen.getByText("Other"));
  expect(screen.getByLabelText("Flow name")).toHaveValue("Release");
  finishSave(flow);
  await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeEnabled());
});

it("shows persisted latest result, error, attempts and next check", async () => {
  vi.mocked(commands.getFlowRun).mockResolvedValue({ ...run, steps: [{ ...run.steps[0], startedAt: "2026-09-14T00:00:00Z", nextCheckAt: "2026-09-14T00:00:02Z", output: { ready: false }, attempts: [{ ...run.steps[0].attempts[0], output: { ready: false }, error: "Transient connection failure" }] }] });
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  fireEvent.click(screen.getByText("Wait", { selector: ".truncate" }));
  expect(await screen.findByText(/Next check/)).toHaveTextContent("2026-09-14T00:00:02Z");
  expect(screen.getByText("Latest result")).toBeInTheDocument();
  expect(screen.getByText(/Latest error/)).toBeInTheDocument();
  expect(screen.getByText(/1 attempts/)).toBeInTheDocument();
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

it("loads and saves an existing definition without Canvas normalization or payload loss", async () => {
  const original = { ...flow, steps: [...flow.steps, { id: "legacy", name: "Legacy", kind: "poll" as const, timeoutMs: 500, intervalMs: 10, maxAttempts: 5, next: "$end", probe: { capability: "api" as const, resourceId: "old", connectionId: null, arguments: { url: { $ref: "/inputs/version" } } }, predicate: { left: true, op: "eq" as const, right: true } }] };
  vi.mocked(commands.listFlows).mockResolvedValue([original]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(screen.getByLabelText("Flow Canvas")).toBeInTheDocument();
  expect(screen.getByLabelText("Input name")).toBeVisible();
  expect(screen.getAllByLabelText("Step name")[0]).not.toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(commands.saveFlow).toHaveBeenCalledWith(original));
});

it("validates manual secret names against runtime inputs without sending optional declared secrets as manual names", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([{ ...flow, inputs: [...flow.inputs, { name: "optionalSecret", type: "string", required: false, secret: true }] }]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("version", { exact: true }), { target: { value: "42" } });
  fireEvent.change(screen.getByLabelText("Secret input names (comma separated)"), { target: { value: "optionalSecret" } });
  expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  expect(screen.getByRole("alert")).toHaveTextContent("must exist");
  fireEvent.change(screen.getByLabelText("Secret input names (comma separated)"), { target: { value: "version" } });
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Run" }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ secretInputNames: ["version"] })));
});

it("projects matching run status onto nodes and hides it after editing", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  await waitFor(() => expect(document.querySelector('.flow-canvas-node[data-status="running"]')).not.toBeNull());
  expect(screen.getByLabelText("Flow name")).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
  fireEvent.change(screen.getByLabelText("Flow name"), { target: { value: "Edited" } });
  await selectHistory();
  expect(document.querySelector('.flow-canvas-node[data-status="running"]')).toBeNull();
});

it("keeps incomplete failure conditions invalid and exposes hidden editor errors", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Add step"), { target: { value: "waitUntil" } });
  fireEvent.click(screen.getByRole("button", { name: "Add condition" }));
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  expect(screen.getByLabelText("Failure when (left / op / right, or null)")).not.toHaveValue('{"left":true,"op":"eq","right":true}');
  fireEvent.click(screen.getByRole("button", { name: "Close inspector" }));
  expect(screen.getByText("Incomplete or invalid configuration. Check these nodes:")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Wait Until", exact: true }));
  fireEvent.change(screen.getByLabelText("Failure condition · Value · Variable"), { target: { value: "/probe/status" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  fireEvent.change(screen.getByLabelText("Failure condition · Operator"), { target: { value: "in" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.getByText("The right operand must be a JSON array or a variable.")).toBeVisible();
  const rightType = screen.getByLabelText("Failure condition · Compare with · Type");
  expect(within(rightType).getByRole("option", { name: "Object / array" })).toBeInTheDocument();
  expect(within(rightType).getByRole("option", { name: "Variable" })).toBeInTheDocument();
  fireEvent.change(rightType, { target: { value: "json" } });
  fireEvent.change(screen.getByLabelText("Failure condition · Compare with", { exact: true }), { target: { value: "[400,404]" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  fireEvent.change(rightType, { target: { value: "variable" } });
  fireEvent.change(screen.getByLabelText("Failure condition · Compare with · Variable"), { target: { value: "/inputs/version" } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

it("keeps selected and failed states together on the same node", async () => {
  vi.mocked(commands.getFlowRun).mockResolvedValue({ ...run, status: "failed", steps: [{ ...run.steps[0], status: "failed" }] });
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  await waitFor(() => expect(document.querySelector('.flow-canvas-node[data-status="failed"]')).not.toBeNull());
  fireEvent.click(document.querySelector('.flow-canvas-node[data-status="failed"]')!);
  expect(document.querySelector('.flow-canvas-node[data-status="failed"]')).toHaveClass("is-selected");
});

it("renders Chinese Condition ports separately from Inspector branch labels", async () => {
  vi.mocked(commands.listFlows).mockResolvedValue([{ ...flow, inputs: [], steps: [{ id: "condition", name: "分支", kind: "condition", timeoutMs: 1000, predicate: { left: 1, op: "eq", right: 1 }, ifTrue: "$end", ifFalse: "$end" }] }]);
  mount("zh-CN");
  fireEvent.click(await screen.findByText("Release"));
  expect(screen.getByText("满足", { selector: ".flow-canvas-port" })).toBeInTheDocument();
  expect(screen.getByText("不满足", { selector: ".flow-canvas-port" })).toBeInTheDocument();
  fireEvent.click(screen.getByText("分支", { selector: ".truncate" }));
  expect(screen.getByLabelText("满足时")).toBeVisible();
  expect(screen.getByLabelText("不满足时")).toBeVisible();
});

it("rejects non-array in operands in Advanced JSON before saving", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.change(screen.getByLabelText("Add step"), { target: { value: "waitUntil" } });
  const field = screen.getByLabelText("Failure when (left / op / right, or null)");
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":"400"}' } });
  expect(field).toHaveAttribute("aria-invalid", "true");
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":400}' } });
  expect(field).toHaveAttribute("aria-invalid", "true");
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":true}' } });
  expect(field).toHaveAttribute("aria-invalid", "true");
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":{"$ref":""}}' } });
  expect(field).toHaveAttribute("aria-invalid", "true");
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":{"$ref":"/inputs/allowedStatuses"}}' } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  fireEvent.change(field, { target: { value: '{"left":400,"op":"in","right":[400,404]}' } });
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

it("loads and saves a legacy in operand whose right value is a $ref array", async () => {
  const original: commands.FlowDefinition = {
    ...flow,
    inputs: [...flow.inputs, { name: "allowedStatuses", type: "json", required: true, secret: false }],
    steps: [{
      id: "ready",
      name: "Ready",
      kind: "waitUntil",
      timeoutMs: 1000,
      intervalMs: 1000,
      maxAttempts: null,
      next: null,
      probe: { capability: "api", resourceId: "probe", connectionId: null, arguments: {} },
      probeErrorPolicy: "failImmediately",
      intervalStrategy: "fixed",
      successWhen: { left: { $ref: "/probe/status" }, op: "eq", right: 200 },
      failureWhen: { left: { $ref: "/probe/status" }, op: "in", right: { $ref: "/inputs/allowedStatuses" } },
    }],
  };
  vi.mocked(commands.listFlows).mockResolvedValue([original]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  fireEvent.click(screen.getByText("Ready", { selector: ".truncate" }));
  expect(screen.getByLabelText("Failure condition · Compare with · Type")).toHaveValue("variable");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(commands.saveFlow).toHaveBeenCalledWith(original));
});


it("keeps runtime fields in the dialog and restores the editor after inspecting a run", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(screen.queryByText("Run inputs & history")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("version", { exact: true })).not.toBeInTheDocument();
  openRun();
  expect(within(screen.getByRole("dialog")).getByLabelText("Environment")).toBeVisible();
  expect(screen.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("version", { exact: true }), { target: { value: "42" } });
  await closeRun();
  expect(commands.runFlow).not.toHaveBeenCalled();
  await selectHistory();
  expect(screen.getByText("Viewing run")).toBeVisible();
  expect(screen.getByLabelText("Flow name")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.queryByLabelText("Add step")).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Wait", { selector: ".truncate" }));
  const inspector = screen.getByRole("complementary", { name: "Run detail" });
  expect((await within(inspector).findByText("Run input values")).nextElementSibling).toHaveTextContent('"version": 42');
  expect(within(inspector).getByText("Latest result")).toBeVisible();
  expect(screen.getByLabelText("Step name")).not.toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
  expect(screen.queryByText("Viewing run")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Step name")).toBeVisible();
  expect(screen.getByLabelText("Flow name")).toBeEnabled();
  expect(document.querySelector(".flow-canvas-node[data-status]")).toBeNull();
  openRun();
  expect(screen.getByLabelText("version", { exact: true })).toHaveValue(42);
});

it("does not project a different revision and shows recorded details explicitly", async () => {
  const previous = { ...run, definition: { ...flow, revision: 2 } };
  vi.mocked(commands.listFlowRuns).mockResolvedValue([summary(previous)]);
  vi.mocked(commands.getFlowRun).mockResolvedValue(previous);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  expect(await screen.findByText(/Node status is hidden/)).toBeVisible();
  expect(document.querySelector(".flow-canvas-node[data-status]")).toBeNull();
  fireEvent.click(screen.getByText("Wait", { selector: ".truncate" }));
  expect(screen.getByRole("complementary", { name: "Run detail" })).toHaveTextContent("r2");
  expect(screen.getByText(/1 attempts/)).toBeVisible();
});

it("lists history status, time and duration with an empty state", async () => {
  vi.mocked(commands.listFlowRuns).mockResolvedValue([summary({ ...run, status: "succeeded", finishedAt: "2026-09-14T00:00:02Z" })]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  const entry = await screen.findByRole("button", { name: /2026-09-14T00:00:00Z/ });
  expect(entry).toHaveTextContent("Succeeded");
  expect(entry).toHaveTextContent("2000 ms");
  fireEvent.click(entry);
  expect(screen.queryByRole("dialog", { name: "Run history" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
  vi.mocked(commands.listFlowRuns).mockResolvedValue([]);
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  expect(await screen.findByText("No runs yet.")).toBeVisible();
});

it("submits the selected environment only after effects confirmation", async () => {
  // Invocation happens before the async result and React's dialog update.
  let finishRun!: (value: commands.FlowRun) => void;
  vi.mocked(commands.runFlow).mockImplementationOnce(() => new Promise((resolve) => { finishRun = resolve; }));
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ id: "prod", name: "Production" }] as Awaited<ReturnType<typeof commands.listWorkspaceEnvironments>>);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("Environment"), { target: { value: "prod" } });
  fireEvent.change(screen.getByLabelText("version", { exact: true }), { target: { value: "42" } });
  expect(screen.getByRole("dialog")).toHaveTextContent("Production");
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: "prod", confirmEffects: true })));
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(screen.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  finishRun(run);
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(await screen.findByText("Viewing run")).toBeVisible();
});

it("blocks invalid runtime JSON inside the dialog and allows correcting it", async () => {
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), { target: { value: "{" } });
  expect(screen.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Run inputs (JSON object)"), { target: { value: '{"version":42}' } });
  expect(screen.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
});


it("keeps removed historical nodes inspectable without projecting them onto the current Canvas", async () => {
  const previous = { ...run, definition: { ...flow, revision: 2, steps: [{ ...flow.steps[0], id: "removed", name: "Old wait" }] }, steps: [{ ...run.steps[0], stepId: "removed" }] };
  vi.mocked(commands.listFlowRuns).mockResolvedValue([summary(previous)]);
  vi.mocked(commands.getFlowRun).mockResolvedValue(previous);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  fireEvent.change(await screen.findByLabelText("Recorded node"), { target: { value: "removed" } });
  expect(screen.getByRole("complementary", { name: "Run detail" })).toHaveTextContent("Old wait");
  expect(screen.getByText(/1 attempts/)).toBeVisible();
  expect(document.querySelector(".flow-canvas-node[data-status]")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
  expect(screen.getByLabelText("Flow name")).toBeEnabled();
});


it("keeps summary failures inside History and clears the alert when closed", async () => {
  vi.mocked(commands.listFlowRuns).mockRejectedValue(new Error("History unavailable"));
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(commands.listFlowRuns).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  const history = await screen.findByRole("dialog", { name: "Run history" });
  await within(history).findByRole("alert");
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  fireEvent.keyDown(history, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(commands.getFlowRun).not.toHaveBeenCalled();
});

it("still shows selected run detail failures on the main page", async () => {
  vi.mocked(commands.getFlowRun).mockRejectedValue(new Error("Detail unavailable"));
  mount();
  fireEvent.click(await screen.findByText("Release"));
  await selectHistory();
  expect(await screen.findByRole("alert")).toBeVisible();
  expect(screen.queryByRole("dialog", { name: "Run history" })).not.toBeInTheDocument();
});

it("loads summaries on opening history and snapshots only after selection", async () => {
  let resolveDetail!: (value: commands.FlowRun) => void;
  vi.mocked(commands.getFlowRun).mockImplementation(() => new Promise((resolve) => { resolveDetail = resolve; }));
  mount();
  fireEvent.click(await screen.findByText("Release"));
  expect(commands.listFlowRuns).not.toHaveBeenCalled();
  expect(commands.getFlowRun).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  const entry = await screen.findByRole("button", { name: /2026-09-14T00:00:00Z/ });
  expect(commands.listFlowRuns).toHaveBeenCalledTimes(1);
  expect(commands.getFlowRun).not.toHaveBeenCalled();
  fireEvent.click(entry);
  await waitFor(() => expect(commands.getFlowRun).toHaveBeenCalledWith("ws", "run-1"));
  expect(document.querySelector(".flow-canvas-node[data-status]")).toBeNull();
  resolveDetail(run);
  await waitFor(() => expect(document.querySelector('.flow-canvas-node[data-status="running"]')).not.toBeNull());
});

it("refreshes open running summaries to completed without fetching detail", async () => {
  vi.mocked(commands.listFlowRuns).mockResolvedValueOnce([summary(run)]).mockResolvedValue([summary({ ...run, status: "succeeded", finishedAt: "2026-09-14T00:00:02Z" })]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  expect(await screen.findByRole("button", { name: /Running.*2026-09-14/ })).toBeVisible();
  expect(await screen.findByRole("button", { name: /Succeeded.*2000 ms/ })).toBeVisible();
  expect(commands.getFlowRun).not.toHaveBeenCalled();
});


it("refreshes a newly completed run in history and stops showing cancellation", async () => {
  const done: commands.FlowRun = { ...run, status: "succeeded", finishedAt: "2026-09-14T00:00:02Z", steps: [{ ...run.steps[0], status: "succeeded" }] };
  vi.mocked(commands.getFlowRun).mockResolvedValueOnce(run).mockResolvedValue(done);
  vi.mocked(commands.listFlowRuns).mockResolvedValue([summary(done)]);
  mount();
  fireEvent.click(await screen.findByText("Release"));
  openRun();
  fireEvent.change(screen.getByLabelText("version", { exact: true }), { target: { value: "42" } });
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  expect(await screen.findByRole("button", { name: "Cancel run" })).toBeVisible();
  await waitFor(() => expect(screen.queryByRole("button", { name: "Cancel run" })).not.toBeInTheDocument());
  expect(commands.listFlowRuns).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Run history" }));
  expect(await screen.findByRole("button", { name: /Succeeded.*2000 ms/ })).toBeVisible();
});
