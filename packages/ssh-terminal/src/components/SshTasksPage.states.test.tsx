// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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
  listSshTaskRuns: vi.fn(),
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
