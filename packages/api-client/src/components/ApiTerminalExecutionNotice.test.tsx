// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createTranslator, I18nProvider } from "@unfour/ui";
import { listTemporaryRoutes } from "@unfour/command-client";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useApiRoutes } from "../hooks/useApiRoutes";
import { createNewRequestTab, emptyApiTabsState, type ApiRequestTab } from "../model/request-tabs";
import { ApiTerminalExecutionNotice } from "./ApiTerminalExecutionNotice";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(),
  getApiRoutes: vi.fn().mockResolvedValue({ request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" }, environmentId: null }),
  listSshConnections: vi.fn().mockResolvedValue([]),
  listTemporaryRoutes: vi.fn().mockResolvedValue([{ handle: "live", displayName: "Node", endpoint: "fixture.invalid:22" }]),
}));
afterEach(cleanup);
const base = createNewRequestTab(emptyApiTabsState("w"), "new:1").tabs[0];
function Notice({ tab, active = true, onUpdate = vi.fn() }: { tab: ApiRequestTab; active?: boolean; onUpdate?: ReturnType<typeof vi.fn> }) {
  const routing = useApiRoutes("w", tab, null, "success");
  return <ApiTerminalExecutionNotice tab={tab} active={active} routing={routing} onUpdate={onUpdate} />;
}
it("keeps recovery warnings visible alongside a received response after changing routes", () => {
  const tab = { ...base, response: { historyId: "h", status: 200, statusText: "OK", headers: [], body: "{}", durationMs: 1,
    warnings: ["SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED"] } };
  render(<QueryClientProvider client={new QueryClient()}><Notice tab={tab} /></QueryClientProvider>);
  expect(screen.getByRole("status")).toHaveTextContent("The HTTP response was received");
  expect(screen.getByRole("status")).toHaveTextContent("Keep this response");
});
it.each(["en", "zh-CN"] as const)("shows the original phase error before the recovery warning in %s", async (locale) => {
  const t = createTranslator(locale);
  vi.mocked(listTemporaryRoutes).mockResolvedValueOnce([{ handle: "live", displayName: "Node", endpoint: "fixture.invalid:22", terminalRecoveryRequired: true }]);
  const tab = { ...base, draft: { ...base.draft, temporaryHandle: "live", temporaryExecutionMode: "terminalCurl" as const },
    sendError: t("network.errors.SSH_TERMINAL_STARTUP_TIMEOUT"), sendErrorCode: "SSH_TERMINAL_STARTUP_TIMEOUT" };
  render(<I18nProvider initialLocale={locale}><QueryClientProvider client={new QueryClient()}><Notice tab={tab} /></QueryClientProvider></I18nProvider>);
  await waitFor(() => expect(screen.getAllByRole("alert")).toHaveLength(2));
  const alerts = screen.getAllByRole("alert");
  expect(alerts[0]).toHaveTextContent(tab.sendError);
  expect(alerts[1]).toHaveTextContent(t("network.errors.SSH_TERMINAL_RECOVERY_REQUIRED"));
});
it("shows remote outcome uncertainty for a cancelled terminal HTTP operation", () => {
  render(<QueryClientProvider client={new QueryClient()}><Notice tab={{ ...base, sendErrorCode: "SSH_TERMINAL_CANCELLED" }} /></QueryClientProvider>);
  expect(screen.getByRole("status")).toHaveTextContent("Remote effects may have occurred");
});
it("invalidates a checked confirmation when the API surface is hidden", async () => {
  const onUpdate = vi.fn();
  const tab = { ...base, draft: { ...base.draft, temporaryHandle: "live", temporaryExecutionMode: "terminalCurl" as const, terminalConfirmed: true } };
  render(<QueryClientProvider client={new QueryClient()}><Notice tab={tab} active={false} onUpdate={onUpdate} /></QueryClientProvider>);
  await waitFor(() => expect(onUpdate).toHaveBeenCalledWith({ terminalConfirmed: false }));
  expect(screen.getByRole("checkbox")).toBeDisabled();
});
