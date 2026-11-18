// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createTranslator, I18nProvider } from "@unfour/ui";
import type { TemporaryProbeReport } from "@unfour/command-client";
import { TemporaryCapabilityDialog } from "./TemporaryCapabilityDialog";
import { serializeTemporaryCapabilityReport } from "./temporary-capability-report";

const api = vi.hoisted(() => ({ start: vi.fn(), get: vi.fn(), app: vi.fn() }));
vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  startTemporarySshProbe: api.start, getTemporarySshProbe: api.get, getAppInfo: api.app,
}));
const report: TemporaryProbeReport = {
  runId: "12345678-1234-4123-8123-123456789abc", kind: "curl", status: "completed",
  startedAt: "2026-11-16T00:00:00Z", finishedAt: "2026-11-16T00:00:01Z", probeFile: null, cleanup: null,
  checks: [
    { id: "transport-roundtrip", code: "SSH_TERMINAL_STARTUP_TIMEOUT", status: "inconclusive", expectedExitCode: 23, actualExitCode: null },
    { id: "transport-recovery", code: "SSH_TERMINAL_RESTORE_TIMEOUT", status: "inconclusive", expectedExitCode: null, actualExitCode: null },
  ],
};
beforeEach(() => { vi.resetAllMocks(); api.get.mockResolvedValue(report); api.app.mockResolvedValue(null); });
afterEach(cleanup);

it.each(["STARTUP", "READY", "ACCEPT", "INPUT_ACK", "BEGIN", "END", "RESTORE"])("retains the %s stage in the sanitized report", (stage) => {
  const code = `SSH_TERMINAL_${stage}_TIMEOUT`;
  const text = serializeTemporaryCapabilityReport([{ ...report, checks: [{ ...report.checks[0], code }] }], null);
  expect(JSON.parse(text).reports[0].checks[0].code).toBe(code);
});

it.each(["en", "zh-CN"] as const)("prioritizes a phase failure and retains the separate recovery code in %s", async (locale) => {
  const t = createTranslator(locale);
  render(<I18nProvider initialLocale={locale}><TemporaryCapabilityDialog workspaceId="w" handle="h" connected terminalRecoveryRequired label="Node" /></I18nProvider>);
  fireEvent.click(screen.getByRole("button", { name: t("ssh.temporary.probe.title") }));
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "curl" } });
  await screen.findByText("SSH_TERMINAL_STARTUP_TIMEOUT");
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent(t("network.errors.SSH_TERMINAL_STARTUP_TIMEOUT"));
  expect(screen.getByText("SSH_TERMINAL_RESTORE_TIMEOUT")).toBeInTheDocument();
  expect(screen.getByText(t("network.errors.SSH_TERMINAL_RESTORE_TIMEOUT"))).toBeInTheDocument();
  const copied = JSON.parse(screen.getByTestId("temporary-capability-report").textContent!);
  expect(copied.reports[0].checks.map((check: { code: string }) => check.code)).toEqual(["SSH_TERMINAL_STARTUP_TIMEOUT", "SSH_TERMINAL_RESTORE_TIMEOUT"]);
  expect(api.start).not.toHaveBeenCalled();
});
