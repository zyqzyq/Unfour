// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createTranslator, I18nProvider } from "@unfour/ui";
import type { TemporaryProbeReport } from "@unfour/command-client";
import { TemporaryCapabilityDialog } from "./TemporaryCapabilityDialog";

const api = vi.hoisted(() => ({ start: vi.fn(), get: vi.fn(), app: vi.fn() }));
vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  startTemporarySshProbe: api.start, getTemporarySshProbe: api.get, getAppInfo: api.app,
}));
const transport: TemporaryProbeReport = {
  runId: "12345678-1234-4123-8123-123456789abc", kind: "transport", status: "completed",
  startedAt: "2026-11-17T00:00:00Z", finishedAt: "2026-11-17T00:00:01Z", probeFile: null, cleanup: null,
  checks: [
    { id: "transport-roundtrip", code: "OK", status: "pass", expectedExitCode: 23, actualExitCode: 23 },
    { id: "transport-recovery", code: "OK", status: "pass", expectedExitCode: null, actualExitCode: null },
  ],
};
beforeEach(() => { vi.resetAllMocks(); api.get.mockResolvedValue(null); api.start.mockResolvedValue(transport); api.app.mockResolvedValue(null); });
afterEach(cleanup);
function view(locale: "en" | "zh-CN" = "en", blocked?: "terminalBusy" | "terminalRecoveryRequired") {
  return <I18nProvider initialLocale={locale}><TemporaryCapabilityDialog workspaceId="w" handle="h" connected label="Node secret"
    terminalBusy={blocked === "terminalBusy"} terminalRecoveryRequired={blocked === "terminalRecoveryRequired"} /></I18nProvider>;
}
async function open(locale: "en" | "zh-CN" = "en") {
  const t = createTranslator(locale);
  fireEvent.click(screen.getByRole("button", { name: t("ssh.temporary.probe.title") }));
  await waitFor(() => expect(screen.queryByText(t("ssh.temporary.probe.runStatus.loading"))).not.toBeInTheDocument());
  return t;
}

it.each(["en", "zh-CN"] as const)("defaults to transport and requires fresh explicit idle-shell confirmation in %s", async (locale) => {
  render(view(locale));
  const t = await open(locale);
  expect(screen.getByRole("combobox")).toHaveValue("transport");
  expect(screen.getByRole("option", { name: t("ssh.temporary.probe.kind.transport") })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: t("ssh.temporary.probe.kind.terminal") })).toHaveTextContent("SSH Task");
  expect(api.start).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox", { name: t("ssh.temporary.probe.authorize") }));
  expect(screen.getByRole("button", { name: t("ssh.temporary.probe.run") })).toBeDisabled();
  const idle = screen.getByRole("checkbox", { name: t("ssh.tasks.run.terminalReady") });
  fireEvent.click(idle);
  fireEvent.click(screen.getByRole("button", { name: t("ssh.temporary.probe.run") }));
  await screen.findByTestId("temporary-capability-report");
  expect(api.start).toHaveBeenCalledExactlyOnceWith("w", "h", "transport", true);
  expect(idle).not.toBeChecked();
  expect(screen.getAllByText(t("ssh.temporary.probe.result.pass"))).toHaveLength(2);
  const report = JSON.parse(screen.getByTestId("temporary-capability-report").textContent!).reports[0];
  expect(report).toMatchObject({ kind: "transport", targetIdentityVerified: false, checks: transport.checks });
  expect(report.checks.some((check: { id: string }) => check.id.startsWith("curl-"))).toBe(false);
});

it.each(["transport", "curl", "terminal"] as const)("invalidates %s confirmation when another operation owns the terminal or recovery becomes required", async (kind) => {
  for (const blocked of ["terminalBusy", "terminalRecoveryRequired"] as const) {
    const mounted = render(view());
    const t = await open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: kind } });
    fireEvent.click(screen.getByRole("checkbox", { name: t("ssh.temporary.probe.authorize") }));
    const idle = screen.getByRole("checkbox", { name: t("ssh.tasks.run.terminalReady") });
    fireEvent.click(idle);
    mounted.rerender(view("en", blocked));
    expect(idle).toBeDisabled();
    expect(idle).not.toBeChecked();
    expect(screen.getByRole("button", { name: t("ssh.temporary.probe.run") })).toBeDisabled();
    mounted.rerender(view());
    expect(idle).toBeEnabled();
    expect(idle).not.toBeChecked();
    expect(screen.getByRole("button", { name: t("ssh.temporary.probe.run") })).toBeDisabled();
    expect(api.start).not.toHaveBeenCalled();
    mounted.unmount();
  }
});

it.each(["en", "zh-CN"] as const)("does not classify a failed transport precheck as unsupported curl in %s", async (locale) => {
  api.get.mockResolvedValue({ ...transport, kind: "curl", checks: [
    { ...transport.checks[0], status: "inconclusive", code: "SSH_TERMINAL_INPUT_ACK_TIMEOUT", actualExitCode: null, raw: "remote-secret" },
    transport.checks[1],
  ] });
  render(view(locale));
  const t = await open(locale);
  expect(screen.getByRole("alert")).toHaveTextContent(t("network.errors.SSH_TERMINAL_INPUT_ACK_TIMEOUT"));
  expect(screen.getByRole("note")).toHaveTextContent(t("ssh.temporary.probe.curlNotChecked"));
  expect(screen.queryByText(t("ssh.temporary.probe.result.unsupported"))).not.toBeInTheDocument();
  expect(screen.queryByText(t("ssh.temporary.probe.check.curl-tools"))).not.toBeInTheDocument();
  const text = screen.getByTestId("temporary-capability-report").textContent!;
  expect(text).not.toMatch(/remote-secret|Node secret|curl-tools|curl-roundtrip/);
  expect(JSON.parse(text).reports[0].checks).toHaveLength(2);
  expect(api.start).not.toHaveBeenCalled();
});

it("prioritizes an unconfirmed transport restoration and leaves curl untested", async () => {
  api.get.mockResolvedValue({ ...transport, kind: "curl", checks: [
    transport.checks[0], { ...transport.checks[1], status: "inconclusive", code: "SSH_TERMINAL_RESTORE_TIMEOUT" },
  ] });
  render(view("en", "terminalRecoveryRequired"));
  const t = await open();
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent(t("ssh.temporary.probe.check.transport-recovery"));
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent(t("network.errors.SSH_TERMINAL_RESTORE_TIMEOUT"));
  expect(screen.getByRole("note")).toHaveTextContent(t("ssh.temporary.probe.curlNotChecked"));
  expect(screen.getAllByText(t("ssh.temporary.probe.result.pass"))).toHaveLength(1);
  expect(screen.getByRole("button", { name: t("ssh.temporary.probe.run") })).toBeDisabled();
  expect(api.start).not.toHaveBeenCalled();
});
