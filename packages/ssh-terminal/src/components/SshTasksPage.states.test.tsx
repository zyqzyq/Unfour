// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { createTranslator, I18nProvider, type Locale } from "@unfour/ui";
import { SshTasksPage } from "./SshTasksPage";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSshTasks: vi.fn(),
  getSshTask: vi.fn(),
  saveSshTask: vi.fn(),
  listSshTaskRuns: vi.fn(),
  readSshTaskRunLog: vi.fn(),
  registerSshTaskRunChannel: vi.fn(async () => () => {}),
}));

const task: commands.SshTask = {
  id: "task", workspaceId: "workspace", name: "Deploy", description: "",
  sortOrder: 0, createdAt: "2026-10-22", updatedAt: "2026-10-22", deletedAt: null,
};
const run: commands.SshTaskRun = {
  id: "run", taskId: task.id, workspaceId: task.workspaceId, connectionId: null,
  status: "failed", startedAt: "2026-10-22T00:00:00Z", finishedAt: null,
  errorMessage: "Connection refused", logPath: "run.log",
};

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(commands.listSshTasks).mockResolvedValue([task]);
  vi.mocked(commands.getSshTask).mockResolvedValue({ task, steps: [], localBinding: null, detectedInputs: [] });
  vi.mocked(commands.listSshTaskRuns).mockResolvedValue([]);
  vi.mocked(commands.readSshTaskRunLog).mockResolvedValue("");
  vi.mocked(commands.registerSshTaskRunChannel).mockResolvedValue(() => {});
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

const onOpenConnections = vi.fn();
function Harness() {
  const [sidebar, setSidebar] = useState<ReactNode>(null);
  return <>
    <aside aria-label="Task sidebar">{sidebar}</aside>
    <SshTasksPage connections={[]} onOpenConnections={onOpenConnections} onShellSidebarChange={setSidebar} workspaceId="workspace" />
  </>;
}

function mount(locale: Locale) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  render(<QueryClientProvider client={client}><I18nProvider initialLocale={locale}><Harness /></I18nProvider></QueryClientProvider>);
  return client;
}

it("refreshes a clean task, preserves a dirty external conflict, and reloads explicitly", async () => {
  const client = mount("en");
  fireEvent.click(await screen.findByRole("button", { name: "Deploy" }));
  const input = await screen.findByRole("textbox", { name: createTranslator("en")("ssh.tasks.editor.name") });
  act(() => client.setQueryData(["ssh-task", "workspace", "task"], { task: { ...task, name: "External" }, steps: [], localBinding: null, detectedInputs: [] }));
  await waitFor(() => expect(input).toHaveValue("External"));
  fireEvent.change(input, { target: { value: "My draft" } });
  act(() => client.setQueryData(["ssh-task", "workspace", "task"], { task: { ...task, name: "External again" }, steps: [], localBinding: null, detectedInputs: [] }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Your draft is retained");
  expect(input).toHaveValue("My draft");
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Load latest version" }));
  expect(input).toHaveValue("External again");
  act(() => client.setQueryData(["ssh-tasks", "workspace"], []));
  expect(await screen.findByRole("alert")).toHaveTextContent("deleted externally");
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
});

it("does not downgrade a saved task when a pre-save detail read finishes late", async () => {
  const client = mount("en");
  fireEvent.click(await screen.findByRole("button", { name: "Deploy" }));
  const input = await screen.findByRole("textbox", { name: createTranslator("en")("ssh.tasks.editor.name") });
  let finishOldRead!: (value: commands.SshTaskDetail) => void;
  const oldDetail = { task, steps: [], localBinding: null, detectedInputs: [] };
  const saved = { ...oldDetail, task: { ...task, name: "Saved draft" } };
  vi.mocked(commands.getSshTask).mockImplementationOnce(() => new Promise((resolve) => { finishOldRead = resolve; })).mockResolvedValue(saved);
  void client.invalidateQueries({ queryKey: ["ssh-task", "workspace", "task"], exact: true });
  await waitFor(() => expect(finishOldRead).toBeDefined());
  fireEvent.change(input, { target: { value: "Saved draft" } });
  vi.mocked(commands.saveSshTask).mockResolvedValue(saved);
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(commands.saveSshTask).toHaveBeenCalledOnce());
  await waitFor(() => expect(commands.getSshTask).toHaveBeenCalledTimes(3));
  await act(async () => finishOldRead(oldDetail));
  expect(input).toHaveValue("Saved draft");
  expect(client.getQueryData(["ssh-task", "workspace", "task"])).toEqual(saved);
});

it.each(["create", "rename"] as const)("shows the saved task during a delayed list refresh (%s)", async (operation) => {
  const client = mount("en");
  const sidebar = within(screen.getByRole("complementary", { name: "Task sidebar" }));
  const existing = await sidebar.findByRole("button", { name: task.name });
  const t = createTranslator("en");
  fireEvent.click(operation === "create" ? sidebar.getByRole("button", { name: t("ssh.tasks.actions.new") }) : existing);
  const name = await screen.findByRole("textbox", { name: t("ssh.tasks.editor.name") });
  const saved = { ...task, id: operation === "create" ? "new" : task.id, name: "Saved task" };
  const detail = { task: saved, steps: [], localBinding: null, detectedInputs: [] };
  const expected = operation === "create" ? [task, saved] : [saved];
  const listKey = ["ssh-tasks", task.workspaceId];
  let finishOldRead!: (rows: commands.SshTask[]) => void;
  let finishRefresh!: (rows: commands.SshTask[]) => void;
  vi.mocked(commands.listSshTasks)
    .mockImplementationOnce(() => new Promise((resolve) => { finishOldRead = resolve; }))
    .mockImplementationOnce(() => new Promise((resolve) => { finishRefresh = resolve; }));
  void client.invalidateQueries({ queryKey: listKey, exact: true });
  await waitFor(() => expect(finishOldRead).toBeDefined());
  vi.mocked(commands.saveSshTask).mockResolvedValue(detail);
  vi.mocked(commands.getSshTask).mockResolvedValue(detail);
  fireEvent.change(name, { target: { value: saved.name } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(finishRefresh).toBeDefined());
  await act(async () => finishOldRead([task]));
  expect(client.getQueryData(listKey)).toEqual(expected);
  expect(await sidebar.findByRole("button", { name: saved.name })).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  await act(async () => finishRefresh(expected));
});

it("refreshes immediately after a save conflict and ignores run-only task metadata", async () => {
  const client = mount("en");
  fireEvent.click(await screen.findByRole("button", { name: "Deploy" }));
  const input = await screen.findByRole("textbox", { name: createTranslator("en")("ssh.tasks.editor.name") });
  fireEvent.change(input, { target: { value: "Draft" } });
  act(() => client.setQueryData(["ssh-task", "workspace", "task"], {
    task: { ...task, sortOrder: 9, updatedAt: "later" }, steps: [], detectedInputs: [],
    localBinding: { taskId: "task", workspaceId: "workspace", defaultConnectionId: null, lastUsedConnectionId: "recent", createdAt: "now", updatedAt: "later" },
  }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  vi.mocked(commands.getSshTask).mockResolvedValue({ task: { ...task, name: "External" }, steps: [], localBinding: null, detectedInputs: [] });
  vi.mocked(commands.saveSshTask).mockRejectedValue({ code: "ROW_CONFLICT", message: "changed" });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByText(/Your draft is retained/)).toBeInTheDocument();
  expect(input).toHaveValue("Draft");
  expect(commands.getSshTask).toHaveBeenCalledTimes(2);
});

it.each(["en", "zh-CN"] as const)("distinguishes a task list failure from an empty workspace and retries (%s)", async (locale) => {
  vi.mocked(commands.listSshTasks).mockRejectedValueOnce(new Error("offline"));
  mount(locale);
  const t = createTranslator(locale);
  const sidebar = within(screen.getByRole("complementary", { name: "Task sidebar" }));
  expect(await sidebar.findByRole("alert")).toHaveTextContent(locale === "en" ? "Unable to load SSH tasks." : "无法加载 SSH Tasks。");
  expect(screen.queryByText(t("ssh.tasks.list.emptyTitle"))).not.toBeInTheDocument();
  fireEvent.click(sidebar.getByRole("button", { name: t("ssh.tasks.actions.retry") }));
  expect(await sidebar.findByRole("button", { name: "Deploy" })).toBeInTheDocument();
  expect(sidebar.queryByRole("alert")).not.toBeInTheDocument();
  expect(commands.listSshTasks).toHaveBeenCalledTimes(2);
});

it.each(["en", "zh-CN"] as const)("localizes task detail failures and retries without closing the tab (%s)", async (locale) => {
  vi.mocked(commands.getSshTask).mockRejectedValueOnce({ code: "IO", message: "offline" });
  mount(locale);
  const t = createTranslator(locale);
  fireEvent.click(await screen.findByRole("button", { name: "Deploy" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(locale === "en" ? "Unable to load this SSH task." : "无法加载此 SSH Task。");
  expect(screen.queryByText("[object Object]")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: t("ssh.tasks.actions.retry") }));
  expect(await screen.findByRole("textbox", { name: t("ssh.tasks.editor.name") })).toHaveValue("Deploy");
  expect(commands.getSshTask).toHaveBeenCalledTimes(2);
});

it.each(["en", "zh-CN"] as const)("distinguishes a history failure from no runs and retries (%s)", async (locale) => {
  vi.mocked(commands.listSshTaskRuns).mockRejectedValueOnce(new Error("offline")).mockResolvedValue([run]);
  mount(locale);
  const t = createTranslator(locale);
  fireEvent.click(await screen.findByRole("button", { name: "Deploy" }));
  await screen.findByRole("textbox", { name: t("ssh.tasks.editor.name") });
  fireEvent.click(screen.getByRole("radio", { name: t("ssh.tasks.tabs.history") }));
  expect(await screen.findByRole("alert")).toHaveTextContent(locale === "en" ? "Unable to load task history." : "无法加载运行历史。");
  expect(screen.queryByText(t("ssh.tasks.history.empty"))).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: t("ssh.tasks.actions.retry") }));
  expect(await screen.findByText("Connection refused")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(commands.listSshTaskRuns).toHaveBeenCalledTimes(2);
});

it.each(["en", "zh-CN"] as const)("creates the Docker example in the selected language (%s)", async (locale) => {
  vi.mocked(commands.listSshTasks).mockResolvedValue([]);
  mount(locale);
  const t = createTranslator(locale);
  fireEvent.click(await within(screen.getByRole("main")).findByRole("button", { name: t("ssh.tasks.actions.dockerExample") }));
  expect(await screen.findByRole("textbox", { name: t("ssh.tasks.editor.name") })).toHaveValue(locale === "en" ? "Docker Image Export" : "Docker 镜像导出");
});

it("settles an MCP-owned output panel after switching tasks and clearing its history", async () => {
  const other = { ...task, id: "other", name: "Build" };
  const running: commands.SshTaskRun = { ...run, status: "running", errorMessage: null };
  const interrupted = { ...run, finishedAt: "2026-10-22T00:01:00Z", errorMessage: "SSH_TASK_RUN_INTERRUPTED" };
  let external = running;
  vi.mocked(commands.listSshTasks).mockResolvedValue([task, other]);
  vi.mocked(commands.getSshTask).mockImplementation(async (_, id) => ({
    task: id === task.id ? task : other, steps: [], localBinding: null, detectedInputs: [],
  }));
  vi.mocked(commands.listSshTaskRuns).mockImplementation(async (_, id) => id === task.id ? [external] : []);
  const client = mount("en");
  const t = createTranslator("en");
  fireEvent.click(await screen.findByRole("button", { name: task.name }));
  fireEvent.click(await screen.findByRole("radio", { name: t("ssh.tasks.tabs.history") }));
  fireEvent.click(await screen.findByRole("button", { name: /run\.log/ }));
  expect(await screen.findByRole("button", { name: t("ssh.tasks.actions.cancelRun") })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: other.name }));
  expect(await screen.findByRole("textbox", { name: t("ssh.tasks.editor.name") })).toHaveValue(other.name);
  fireEvent.click(screen.getByRole("radio", { name: t("ssh.tasks.tabs.history") }));
  expect(await screen.findByText(t("ssh.tasks.history.empty"))).toBeInTheDocument();
  external = interrupted;
  await screen.findByText(t("ssh.tasks.run.interrupted"), {}, { timeout: 4_000 });
  act(() => client.setQueryData(["ssh-task-runs", task.workspaceId, task.id], []));
  expect(screen.queryByRole("button", { name: t("ssh.tasks.actions.cancelRun") })).not.toBeInTheDocument();
  expect(screen.getByText(t("ssh.tasks.run.interrupted"))).toBeInTheDocument();
  expect(screen.getByText(t("ssh.tasks.history.empty"))).toBeInTheDocument();
});

it("confirms a missing running row before closing its output", async () => {
  const running: commands.SshTaskRun = { ...run, status: "running", errorMessage: null };
  vi.mocked(commands.listSshTaskRuns).mockResolvedValue([running]);
  const client = mount("en");
  const t = createTranslator("en");
  fireEvent.click(await screen.findByRole("button", { name: task.name }));
  fireEvent.click(await screen.findByRole("radio", { name: t("ssh.tasks.tabs.history") }));
  fireEvent.click(await screen.findByRole("button", { name: /run\.log/ }));
  const cancel = await screen.findByRole("button", { name: t("ssh.tasks.actions.cancelRun") });
  const key = ["ssh-task-runs", task.workspaceId, task.id];
  act(() => client.setQueryData(key, []));
  await waitFor(() => expect(client.getQueryData(key)).toEqual([running]));
  expect(cancel).toBeInTheDocument();
  vi.mocked(commands.listSshTaskRuns).mockResolvedValue([]);
  await act(async () => { await client.invalidateQueries({ queryKey: key }); });
  await waitFor(() => expect(cancel).not.toBeInTheDocument());
});
