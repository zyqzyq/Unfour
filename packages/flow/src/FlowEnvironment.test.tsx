// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useState, type ReactNode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { FlowPage } from "./FlowPage";

vi.mock("@unfour/command-client", async (original) => ({
  ...(await original<typeof import("@unfour/command-client")>()),
  listFlows: vi.fn(), listFlowRuns: vi.fn(), getFlowRun: vi.fn(), runFlow: vi.fn(),
  listSavedApiRequests: vi.fn(), listSshTasks: vi.fn(), listDatabaseConnections: vi.fn(),
  listSshConnections: vi.fn(), listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(),
}));
const flow: commands.FlowDefinition = {
  id: "flow", workspaceId: "ws", name: "Release", revision: 3, inputs: [],
  steps: [{ id: "wait", name: "Wait", kind: "wait", durationMs: 1, timeoutMs: 1000, next: null }],
};
const run: commands.FlowRun = {
  id: "run", workspaceId: "ws", flowId: flow.id, definition: flow,
  context: { workspaceId: "ws", flowId: flow.id, environmentId: null, inputs: {}, initiator: "human", confirmEffects: true },
  status: "succeeded", startedAt: "2026-10-14T00:00:00Z", finishedAt: "2026-10-14T00:00:01Z",
  resources: {}, error: null, steps: [],
};
function environment(id: string, isActive = false, workspaceId = "ws"): commands.WorkspaceEnvironment {
  return { id, name: `Name of ${id}`, workspaceId, isActive, sortOrder: 0, variables: [],
    createdAt: "", updatedAt: "", deletedAt: null, revision: 1 };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(commands.listFlows).mockImplementation(async (workspaceId) => [{ ...flow, workspaceId }]);
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([environment("dev", true), environment("prod")]);
  vi.mocked(commands.listFlowRuns).mockResolvedValue([run]);
  vi.mocked(commands.getFlowRun).mockResolvedValue(run);
  vi.mocked(commands.runFlow).mockResolvedValue(run);
  for (const command of [commands.listSavedApiRequests, commands.listSshTasks,
    commands.listDatabaseConnections, commands.listSshConnections, commands.listWorkspaceVariables])
    vi.mocked(command).mockResolvedValue([]);
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
const copy = {
  en: { run: "Run", cancel: "Cancel", environment: "Variable environment", only: "Workspace variables only", recorded: "Variable environment used (ID)" },
  "zh-CN": { run: "运行", cancel: "取消", environment: "变量环境", only: "仅 Workspace 变量", recorded: "使用的变量环境（ID）" },
};
function Harness({ workspaceId }: { workspaceId: string }) {
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return <>{sidebar}<FlowPage workspaceId={workspaceId} onSidebarContentChange={setSidebar} /></>;
}
function mount(locale: keyof typeof copy = "en") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  function tree(workspaceId: string) {
    return <QueryClientProvider client={client}><I18nProvider initialLocale={locale}>
      <Harness workspaceId={workspaceId} />
    </I18nProvider></QueryClientProvider>;
  }
  const result = render(tree("ws"));
  return { client, switchWorkspace: (id: string) => result.rerender(tree(id)) };
}
async function open(locale: keyof typeof copy = "en") {
  fireEvent.click(await screen.findByRole("button", { name: "Release", exact: true }));
  fireEvent.click(screen.getByRole("button", { name: copy[locale].run, exact: true }));
  return within(screen.getByRole("dialog"));
}
function deferredEnvironments() {
  let resolve!: (items: commands.WorkspaceEnvironment[]) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<commands.WorkspaceEnvironment[]>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
async function openWithEnvironmentFailure() {
  const initial = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(initial.promise);
  const mounted = mount();
  const dialog = await open();
  await act(async () => initial.reject(new Error("Unavailable")));
  await dialog.findByRole("alert");
  return { ...mounted, dialog };
}

it.each([
  { items: [environment("dev", true)], expected: "dev" },
  { items: [environment("dev")], expected: "" },
  { items: [], expected: "" },
])("restores the default only after a successful retry (%j)", async ({ items, expected }) => {
  const { dialog } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(retry.promise);
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  expect(commands.runFlow).not.toHaveBeenCalled();
  await act(async () => retry.resolve(items));
  await waitFor(() => expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue(expected);
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: expected || null, confirmEffects: true }), 3));
});

it.each(["before retry", "during retry"])("preserves an explicit workspace-only choice %s", async (when) => {
  const { dialog } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(retry.promise);
  const chooseWorkspace = () => fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: "" } });
  if (when === "before retry") chooseWorkspace();
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  if (when === "during retry") chooseWorkspace();
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  await act(async () => retry.resolve([environment("dev", true)]));
  await waitFor(() => expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("");
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: null }), 3));
});

it.each(["", "prod"])("preserves a manual cached selection across a failed refresh and retry (%s)", async (selected) => {
  const { client } = mount();
  const dialog = await open();
  fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: selected } });
  vi.mocked(commands.listWorkspaceEnvironments).mockRejectedValueOnce(new Error("Unavailable"));
  await act(async () => client.refetchQueries({ queryKey: ["workspace-environments", "ws"] }));
  await dialog.findByRole("alert");
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  await waitFor(() => expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue(selected);
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it("recovers an untouched default when the shared query refresh succeeds without the Retry button", async () => {
  const { client, dialog } = await openWithEnvironmentFailure();
  await act(async () => client.refetchQueries({ queryKey: ["workspace-environments", "ws"] }));
  await waitFor(() => expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("dev");
  act(() => client.setQueryData(["workspace-environments", "ws"], [{ ...environment("prod", true), name: "Refreshed Production" }, environment("dev")]));
  await dialog.findByRole("option", { name: "Refreshed Production" });
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("dev");
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it.each([false, true])("uses the reopened dialog's choice when an earlier retry returns (%s)", async (manual) => {
  const { dialog } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(retry.promise);
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  fireEvent.click(dialog.getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  if (manual) fireEvent.change(screen.getByLabelText(copy.en.environment), { target: { value: "" } });
  expect(screen.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  await act(async () => retry.resolve([environment("dev", true)]));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue(manual ? "" : "dev");
  expect(commands.listWorkspaceEnvironments).toHaveBeenCalledTimes(2);
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it("ignores an old retry result after reopening with a newer cached default", async () => {
  const { client, dialog } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(retry.promise);
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  fireEvent.click(dialog.getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  act(() => client.setQueryData(["workspace-environments", "ws"], [environment("prod", true), environment("dev")]));
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue("prod");
  await act(async () => retry.resolve([environment("dev", true), environment("prod")]));
  await waitFor(() => expect(client.getQueryData<commands.WorkspaceEnvironment[]>(["workspace-environments", "ws"])?.find((item) => item.isActive)?.id).toBe("dev"));
  await waitFor(() => expect(within(screen.getByRole("dialog")).getAllByRole("option").map((option) => option.getAttribute("value"))).toEqual(["", "dev", "prod"]));
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue("prod");
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it.each([false, true])("resets the default when switching Flow while a retry is pending (%s)", async (manual) => {
  vi.mocked(commands.listFlows).mockResolvedValue([flow, { ...flow, id: "other-flow", name: "Other" }]);
  const { dialog } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValueOnce(retry.promise);
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: "" } });
  fireEvent.click(dialog.getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "Other", exact: true }));
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  if (manual) fireEvent.change(screen.getByLabelText(copy.en.environment), { target: { value: "" } });
  await act(async () => retry.resolve([environment("dev", true)]));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue(manual ? "" : "dev");
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ flowId: "other-flow", environmentId: manual ? null : "dev" }), 3));
});

it("ignores an old Workspace retry result after switching Workspace", async () => {
  const { client, dialog, switchWorkspace } = await openWithEnvironmentFailure();
  const retry = deferredEnvironments();
  vi.mocked(commands.listWorkspaceEnvironments).mockImplementation((workspaceId) => workspaceId === "ws"
    ? retry.promise : Promise.resolve([environment("other", true, workspaceId)]));
  fireEvent.click(dialog.getByRole("button", { name: "Retry", exact: true }));
  switchWorkspace("other-ws");
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  const currentDialog = await open();
  await act(async () => retry.resolve([environment("dev", true)]));
  await waitFor(() => expect(client.getQueryData<commands.WorkspaceEnvironment[]>(["workspace-environments", "ws"])?.[0].id).toBe("dev"));
  expect(currentDialog.getByLabelText(copy.en.environment)).toHaveValue("other");
  fireEvent.click(currentDialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ workspaceId: "other-ws", environmentId: "other" }), 3));
});

it.each(["en", "zh-CN"] as const)("prefills the active environment and submits it only after confirmation (%s)", async (locale) => {
  mount(locale);
  const dialog = await open(locale);
  expect(dialog.getByLabelText(copy[locale].environment)).toHaveValue("dev");
  expect(dialog.getByText(`${copy[locale].environment}: Name of dev (dev)`)).toBeVisible();
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.click(dialog.getByRole("button", { name: copy[locale].run, exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith({
    workspaceId: "ws", flowId: "flow", environmentId: "dev", inputs: {},
    secretInputNames: [], initiator: "human", confirmEffects: true,
  }, 3));
});

it.each([{ environments: [] }, { environments: [environment("dev")] }])("uses workspace-only when no environment is active (%j)", async ({ environments }) => {
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue(environments);
  mount();
  const dialog = await open();
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("");
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: null }), 3));
});

it.each(["", "prod", null])("preserves the open dialog's selection through active-environment changes and refetches (%s)", async (manual) => {
  const { client } = mount();
  const dialog = await open();
  if (manual !== null) fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: manual } });
  const selected = manual ?? "dev";
  const updated = [environment("dev"), environment("prod", true)];
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue(updated);
  await act(async () => {
    client.setQueryData(["workspace-environments", "ws"], updated);
    await client.refetchQueries({ queryKey: ["workspace-environments", "ws"] });
  });
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue(selected);
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: selected || null }), 3));
});

it.each([false, true])("initializes after an initial load only if no manual choice was made (%s)", async (manual) => {
  let resolve!: (items: commands.WorkspaceEnvironment[]) => void;
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValue(new Promise((done) => { resolve = done; }));
  mount();
  const dialog = await open();
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  if (manual) fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: "" } });
  await act(async () => resolve([environment("dev", true)]));
  await waitFor(() => expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue(manual ? "" : "dev");
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it("snapshots a fresh default on reopen and run again", async () => {
  const { client } = mount();
  const dialog = await open();
  fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: "" } });
  fireEvent.click(dialog.getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  act(() => client.setQueryData(["workspace-environments", "ws"], [environment("prod", true)]));
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue("prod");
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  await screen.findByText("Viewing run");
  act(() => client.setQueryData(["workspace-environments", "ws"], [environment("dev", true)]));
  fireEvent.click(screen.getByRole("button", { name: "Run again", exact: true }));
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue("dev");
  expect(commands.runFlow).toHaveBeenCalledTimes(1);
});

it("does not let earlier pending opens replace a manual workspace-only choice after reopening", async () => {
  let resolve!: (items: commands.WorkspaceEnvironment[]) => void;
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValue(new Promise((done) => { resolve = done; }));
  mount();
  const first = await open();
  fireEvent.click(first.getByRole("button", { name: "Cancel", exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "Run", exact: true }));
  fireEvent.change(screen.getByLabelText(copy.en.environment), { target: { value: "" } });
  await act(async () => resolve([environment("dev", true)]));
  await waitFor(() => expect(screen.getByRole("button", { name: "Run", exact: true })).toBeEnabled());
  expect(screen.getByLabelText(copy.en.environment)).toHaveValue("");
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it("keeps execution blocked if the environment query fails during initialization", async () => {
  let reject!: (error: Error) => void;
  vi.mocked(commands.listWorkspaceEnvironments).mockReturnValue(new Promise((_resolve, fail) => { reject = fail; }));
  mount();
  const dialog = await open();
  await act(async () => reject(new Error("Unavailable")));
  expect(await dialog.findByRole("alert")).toHaveTextContent("Unable to load Flow resources or history");
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  expect(commands.runFlow).not.toHaveBeenCalled();
});

it("keeps a deleted selection explicit and blocks execution until manually changed", async () => {
  const { client } = mount();
  const dialog = await open();
  act(() => client.setQueryData(["workspace-environments", "ws"], [environment("prod", true)]));
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("dev");
  expect(await dialog.findByText("Variable environment: dev")).toBeVisible();
  expect(dialog.getByRole("alert")).toHaveTextContent("The selected environment is unavailable");
  expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  expect(commands.runFlow).not.toHaveBeenCalled();
  fireEvent.change(dialog.getByLabelText(copy.en.environment), { target: { value: "" } });
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ environmentId: null }), 3));
});

it("discards pending environment initialization when switching Workspace", async () => {
  let resolve!: (items: commands.WorkspaceEnvironment[]) => void;
  vi.mocked(commands.listWorkspaceEnvironments).mockImplementation((workspaceId) => workspaceId === "ws"
    ? new Promise((done) => { resolve = done; }) : Promise.resolve([environment("other", true, workspaceId)]));
  const { switchWorkspace } = mount();
  await open();
  switchWorkspace("other-ws");
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  const dialog = await open();
  await act(async () => resolve([environment("dev", true)]));
  expect(dialog.getByLabelText(copy.en.environment)).toHaveValue("other");
  fireEvent.click(dialog.getByRole("button", { name: "Run", exact: true }));
  await waitFor(() => expect(commands.runFlow).toHaveBeenCalledWith(expect.objectContaining({ workspaceId: "other-ws", environmentId: "other" }), 3));
});

it.each(["en", "zh-CN"] as const)("shows recorded IDs, never current names or active defaults, in history (%s)", async (locale) => {
  const { client } = mount(locale);
  await open(locale);
  fireEvent.click(screen.getByRole("button", { name: copy[locale].cancel, exact: true }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: locale === "en" ? "Run history" : "运行历史" }));
  fireEvent.click(await screen.findByRole("button", { name: /2026-10-14T00:00:00Z/ }));
  const inspector = within(screen.getByRole("complementary", { name: locale === "en" ? "Run detail" : "运行详情" }));
  await inspector.findByText(`${copy[locale].recorded}: ${copy[locale].only}`);
  for (const environmentId of ["dev", "deleted-id", null, undefined]) {
    act(() => client.setQueryData(["flow-run", "ws", run.id], { ...run, context: { ...run.context, environmentId } }));
    expect(await inspector.findByText(`${copy[locale].recorded}: ${environmentId ?? copy[locale].only}`)).toBeVisible();
    expect(inspector.queryByText(/Name of/)).not.toBeInTheDocument();
  }
});
