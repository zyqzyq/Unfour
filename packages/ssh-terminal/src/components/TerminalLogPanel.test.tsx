// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { exportSshLog } from "@unfour/command-client";
import { useTerminalStore } from "../model/terminal-state";
import { TerminalLogPanel } from "./TerminalLogPanel";

vi.mock("@unfour/command-client", () => ({ exportSshLog: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  useTerminalStore.setState({ activeSessionId: "session", exportedLog: null, terminalEvents: [
    { sessionId: "session", kind: "output", data: "ready\nauthorization: fixture-secret", createdAt: "2026-10-14" },
  ] });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
function mount(locale: "en" | "zh-CN" = "en") {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><I18nProvider initialLocale={locale}>
    <TerminalLogPanel workspaceId="ws" onCollapse={vi.fn()} onHeightChange={vi.fn()} />
  </I18nProvider></QueryClientProvider>);
}

it("copies only redacted text, reports failure, and supports retry", async () => {
  const writeText = vi.fn().mockRejectedValueOnce(new Error("Denied")).mockResolvedValue(undefined);
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  mount();
  const copy = screen.getByRole("button", { name: "Copy terminal logs" });
  fireEvent.click(copy);
  expect(await screen.findByRole("alert")).toHaveTextContent("Check clipboard access");
  fireEvent.click(copy);
  expect(await screen.findByRole("status")).toHaveTextContent("Logs copied");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(writeText).toHaveBeenLastCalledWith("[2026-10-14] output ready\n<redacted>");
  expect(screen.queryByText(/fixture-secret/)).not.toBeInTheDocument();
});

it("reports an unavailable clipboard instead of silently succeeding", async () => {
  vi.stubGlobal("navigator", {});
  mount("zh-CN");
  fireEvent.click(screen.getByRole("button", { name: "复制终端日志" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("请检查剪贴板权限后重试");
});

it("prevents duplicate copy requests while the clipboard is pending", async () => {
  vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn(() => new Promise(() => {})) } });
  mount();
  const copy = screen.getByRole("button", { name: "Copy terminal logs" });
  fireEvent.click(copy);
  await waitFor(() => expect(copy).toBeDisabled());
  fireEvent.click(copy);
  expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1);
});

it("localizes the empty state and leaves unavailable actions disabled", () => {
  useTerminalStore.setState({ activeSessionId: null, terminalEvents: [] });
  mount("zh-CN");
  expect(screen.getByText("连接事件")).toBeVisible();
  expect(screen.getByText("暂无连接事件")).toBeVisible();
  expect(screen.getByRole("button", { name: "复制终端日志" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "导出终端日志" })).toBeDisabled();
});

it("keeps logs on export failure and recovers through the existing export action", async () => {
  vi.mocked(exportSshLog).mockRejectedValueOnce(new Error("Unavailable")).mockResolvedValue({ sessionId: "session", content: "exported\ncookie: fixture-secret" });
  mount("zh-CN");
  fireEvent.click(screen.getByRole("button", { name: "导出终端日志" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("日志导出失败");
  expect(screen.getByText(/output ready/)).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "导出终端日志" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  expect(screen.getByText(/exported/)).toHaveTextContent("<redacted>");
  expect(exportSshLog).toHaveBeenLastCalledWith({ workspaceId: "ws", sessionId: "session" });
});
