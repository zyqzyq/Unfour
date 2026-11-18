// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import type { AppInfo, TemporaryProbeReport } from "@unfour/command-client";
import { TemporaryCapabilityDialog } from "./TemporaryCapabilityDialog";
import { serializeTemporaryCapabilityReport, temporaryTcpTarget } from "./temporary-capability-report";

const api = vi.hoisted(() => ({ start: vi.fn(), tcp: vi.fn(), get: vi.fn(), cancel: vi.fn(), app: vi.fn(), copy: vi.fn(), terminalCancel: vi.fn() }));
vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  startTemporarySshProbe: api.start, getTemporarySshProbe: api.get, cancelTemporarySshProbe: api.cancel,
  startTemporarySshTcpProbe: api.tcp,
  getAppInfo: api.app, cancelTemporarySsh: api.terminalCancel,
}));
const runId = "12345678-1234-4123-8123-123456789abc";
const report: TemporaryProbeReport = {
  runId, kind: "exec", status: "completed", startedAt: "2026-11-16T00:00:00Z", finishedAt: "2026-11-16T00:00:01Z",
  checks: [{ id: "exec-success", code: "OK", status: "pass", expectedExitCode: 0, actualExitCode: 0 }], probeFile: null, cleanup: null,
};
const app: AppInfo = { name: "Unfour", version: "0.14.0", commit: "1234567-dirty", channel: "test", distribution: "standard" };
function view(workspaceId = "workspace-a", handle = "handle-a", connected = true) {
  return <I18nProvider initialLocale="en"><TemporaryCapabilityDialog workspaceId={workspaceId} handle={handle} connected={connected} label="Node secret · 10.2.3.4:22" /></I18nProvider>;
}
async function open() {
  fireEvent.click(screen.getByRole("button", { name: "Capability checks" }));
  await waitFor(() => expect(screen.queryByText("Loading check status…")).not.toBeInTheDocument());
  // These existing lifecycle cases exercise exec explicitly. The transport
  // default and its extra idle-shell confirmation have dedicated coverage.
  const select = screen.getByRole("combobox");
  if (!select.hasAttribute("disabled")) fireEvent.change(select, { target: { value: "exec" } });
}
function authorize() { fireEvent.click(screen.getByRole("checkbox", { name: "I confirm this is an authorized test asset." })); }

describe("manual temporary capability checks", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.get.mockResolvedValue(null); api.start.mockResolvedValue(report); api.cancel.mockResolvedValue(undefined); api.app.mockResolvedValue(app); api.copy.mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: api.copy } });
  });
  afterEach(() => { cleanup(); vi.useRealTimers(); });

  it("requires logged-in shell confirmation and reports terminal checks without claiming host identity", async () => {
    api.start.mockResolvedValue({ ...report, kind: "terminal", checks: [{ ...report.checks[0], id: "terminal-success" }] });
    render(view()); await open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "terminal" } });
    authorize();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    const ready = screen.getByRole("checkbox", { name: /I have entered the intended host/ });
    fireEvent.click(ready);
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await screen.findByText("Passed");
    expect(api.start).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", "terminal", true);
    expect(ready).not.toBeChecked();
    const copied = JSON.parse(screen.getByTestId("temporary-capability-report").textContent!);
    expect(copied.reports[0]).toMatchObject({ kind: "terminal", targetIdentityVerified: false, checks: [{ id: "terminal-success" }] });
  });
  it("checks curl without an HTTP target and requires fresh idle-shell confirmation", async () => {
    api.start.mockResolvedValue({ ...report, kind: "curl", checks: [
      { ...report.checks[0], id: "transport-roundtrip", expectedExitCode: 23, actualExitCode: 23 },
      { ...report.checks[0], id: "transport-recovery", expectedExitCode: null, actualExitCode: null },
      { ...report.checks[0], id: "curl-tools" },
      { ...report.checks[0], id: "curl-recovery", expectedExitCode: null, actualExitCode: null },
    ] });
    render(view()); await open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "curl" } });
    expect(screen.getByText(/without contacting an HTTP service/)).toBeInTheDocument();
    authorize();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    const ready = screen.getByRole("checkbox", { name: /I have entered the intended host/ });
    fireEvent.click(ready);
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await screen.findByText("curl tools and options");
    expect(api.start).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", "curl", true);
    expect(api.tcp).not.toHaveBeenCalled();
    expect(ready).not.toBeChecked();
    expect(JSON.parse(screen.getByTestId("temporary-capability-report").textContent!).reports[0]).toMatchObject({ kind: "curl", targetIdentityVerified: false, checks: [
      { id: "transport-roundtrip" }, { id: "transport-recovery" }, { id: "curl-tools" }, { id: "curl-recovery" },
    ] });
  });
  it.each(["CURL_MISSING", "TERMINAL_TOOLS_MISSING", "CURL_OPTIONS_UNSUPPORTED", "TERMINAL_PROTOCOL_ERROR"])("preserves the fixed curl code %s without copying server text", async (code) => {
    api.get.mockResolvedValue({ ...report, kind: "curl", checks: [{ ...report.checks[0], id: "curl-tools", code, status: "unsupported", raw: "server-secret" }] });
    render(view()); await open();
    expect(screen.getByText(code)).toBeInTheDocument();
    expect(screen.getByTestId("temporary-capability-report").textContent).not.toContain("server-secret");
  });
  it("keeps terminal checks disabled when the parent requires recovery", async () => {
    render(<I18nProvider initialLocale="en"><TemporaryCapabilityDialog workspaceId="w" handle="h" connected terminalRecoveryRequired label="Node" /></I18nProvider>);
    await open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "terminal" } });
    authorize();
    expect(screen.getByRole("checkbox", { name: /I have entered the intended host/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Close this tab and log in again");
    expect(api.start).not.toHaveBeenCalled();
  });
  it("shows an incomplete terminal setup without claiming a passed capability", async () => {
    api.get.mockResolvedValue({ ...report, kind: "terminal", checks: [{ ...report.checks[0], id: "terminal-success", status: "inconclusive", code: "TERMINAL_NOT_READY", actualExitCode: null }] });
    render(view()); await open();
    expect(screen.queryByText("Passed")).not.toBeInTheDocument();
    expect(screen.getByText("TERMINAL_NOT_READY")).toBeInTheDocument();
    expect(screen.getByText(/The terminal did not confirm an idle shell/)).toBeInTheDocument();
    expect(JSON.parse(screen.getByTestId("temporary-capability-report").textContent!).reports[0].checks[0].code).toBe("TERMINAL_NOT_READY");
  });
  it("preserves a separate failed terminal recovery result", async () => {
    api.get.mockResolvedValue({ ...report, kind: "terminal", checks: [
      { ...report.checks[0], id: "terminal-success" },
      { id: "terminal-recovery", status: "inconclusive", code: "TERMINAL_RECOVERY_REQUIRED", expectedExitCode: null, actualExitCode: null },
    ] });
    render(view()); await open();
    expect(screen.getByText("Terminal recovery")).toBeInTheDocument();
    expect(screen.getByText(/The terminal state could not be confirmed/)).toBeInTheDocument();
    expect(JSON.parse(screen.getByTestId("temporary-capability-report").textContent!).reports[0].checks[1]).toEqual({
      id: "terminal-recovery", status: "inconclusive", code: "TERMINAL_RECOVERY_REQUIRED", expectedExitCode: null, actualExitCode: null,
    });
  });

  it("checks only the explicit TCP target and excludes it from the report", async () => {
    const tcp = { ...report, kind: "tcp", checks: [{ id: "tcp-forward", status: "unsupported", code: "SSH_TUNNEL_FORWARD_PROHIBITED", expectedExitCode: null, actualExitCode: null }], host: "10.91.92.93", port: 18291 };
    api.tcp.mockResolvedValue(tcp);
    render(view()); await open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "tcp" } });
    fireEvent.change(screen.getByLabelText("Target host or IP"), { target: { value: "http://10.91.92.93/path" } });
    fireEvent.change(screen.getByLabelText("Port"), { target: { value: "18291" } });
    authorize();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Target host or IP"), { target: { value: "10.91.92.93" } });
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    authorize();
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await screen.findByText("Unsupported");
    expect(api.tcp).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", "10.91.92.93", 18291, true);
    expect(api.start).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Target host or IP")).toHaveValue("");
    expect(screen.getByTestId("temporary-capability-report").textContent).not.toMatch(/10\.91\.92\.93|18291/);
    expect(screen.getByText(/The bastion denied TCP forwarding/)).toBeInTheDocument();
  });

  it("requires a deliberate start and renewed confirmation for each kind", async () => {
    render(view());
    expect(api.get).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
    await open();
    expect(api.start).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    authorize();
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await screen.findByText("Passed");
    expect(api.start).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", "exec", true);
    expect(screen.getByRole("checkbox", { name: "I confirm this is an authorized test asset." })).not.toBeChecked();
    fireEvent.change(screen.getByRole("combobox", { name: "Check to run" }), { target: { value: "sftp" } });
    authorize();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "I authorize creating, reading and deleting the probe file in /tmp." }));
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await waitFor(() => expect(api.start).toHaveBeenLastCalledWith("workspace-a", "handle-a", "sftp", true));
  });

  it("prevents overlapping starts and stops only the selected probe", async () => {
    api.start.mockResolvedValue({ ...report, status: "running", finishedAt: null });
    render(view()); await open(); authorize();
    const run = screen.getByRole("button", { name: "Run check" });
    act(() => { fireEvent.click(run); fireEvent.click(run); });
    await screen.findByRole("button", { name: "Stop check" });
    expect(api.start).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status")).toHaveTextContent("Check running…");
    expect(screen.getByRole("combobox")).toBeDisabled();
    api.get.mockResolvedValue({ ...report, status: "cancelled", checks: [{ ...report.checks[0], status: "cancelled", code: "CANCELLED" }] });
    fireEvent.click(screen.getByRole("button", { name: "Stop check" }));
    await screen.findByText("Check cancelled");
    expect(api.cancel).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", runId);
    expect(api.terminalCancel).not.toHaveBeenCalled();
  });

  it("stops a start that finishes after the dialog closes", async () => {
    let finish!: (value: TemporaryProbeReport) => void;
    api.start.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    render(view()); await open(); authorize();
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Close" }).at(-1)!);
    await act(async () => finish({ ...report, status: "running", finishedAt: null }));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledWith("workspace-a", "handle-a", runId));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.terminalCancel).not.toHaveBeenCalled();
  });

  it("honors Stop while a start is still pending", async () => {
    let finish!: (value: TemporaryProbeReport) => void;
    api.start.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    render(view()); await open(); authorize();
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop check" }));
    expect(screen.getByRole("button", { name: "Stop check" })).toBeDisabled();
    api.get.mockResolvedValue({ ...report, status: "cancelled" });
    await act(async () => finish({ ...report, status: "running", finishedAt: null }));
    await screen.findByText("Check cancelled");
    expect(api.cancel).toHaveBeenCalledWith("workspace-a", "handle-a", runId);
  });

  it("drops late reads and clears authorizations when workspace or session changes", async () => {
    let finish!: (value: TemporaryProbeReport) => void;
    api.get.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const mounted = render(view());
    fireEvent.click(screen.getByRole("button", { name: "Capability checks" }));
    mounted.rerender(view("workspace-b", "handle-b"));
    await open();
    await act(async () => finish(report));
    expect(screen.queryByText("Passed")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    authorize(); fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    await waitFor(() => expect(api.start).toHaveBeenCalledWith("workspace-b", "handle-b", "exec", true));
  });

  it("copies an allowlisted in-memory report only on request", async () => {
    api.get.mockResolvedValue({ ...report, endpoint: "10.2.3.4", username: "user-secret", output: "raw-secret" });
    render(view()); await open();
    expect(api.copy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Copy sanitized report" }));
    await screen.findByRole("button", { name: "Report copied" });
    const text = api.copy.mock.calls[0][0] as string;
    expect(text).not.toMatch(/10\.2\.3\.4|Node secret|user-secret|raw-secret|workspace-a|handle-a/);
    expect(JSON.parse(text)).toMatchObject({ app: { version: "0.14.0", commit: "1234567-dirty" }, reports: [{ runId, kind: "exec" }] });
  });

  it("cancels an existing check when its session leaves the surface", async () => {
    api.get.mockResolvedValue({ ...report, status: "running", finishedAt: null });
    const mounted = render(view()); await open();
    mounted.rerender(view("workspace-a", "handle-b"));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledExactlyOnceWith("workspace-a", "handle-a", runId));
    expect(api.terminalCancel).not.toHaveBeenCalled();
    expect(api.start).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("does not report an error when the parent was already closed during cleanup", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => undefined);
    api.get.mockResolvedValue({ ...report, status: "running", finishedAt: null });
    api.cancel.mockRejectedValue(new Error("validation error: TEMPORARY_NOT_FOUND"));
    const mounted = render(view()); await open();
    mounted.unmount();
    await act(async () => undefined);
    expect(api.cancel).toHaveBeenCalledWith("workspace-a", "handle-a", runId);
    expect(log).not.toHaveBeenCalled();
    log.mockRestore();
  });

  it("does not show a late copy acknowledgement after the dialog reopens", async () => {
    let finish!: () => void;
    api.copy.mockReturnValue(new Promise<void>((resolve) => { finish = resolve; }));
    api.get.mockResolvedValue(report);
    render(view()); await open();
    fireEvent.click(screen.getByRole("button", { name: "Copy sanitized report" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Close" }).at(-1)!);
    await open();
    await act(async () => finish());
    expect(screen.queryByRole("button", { name: "Report copied" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy sanitized report" })).toBeEnabled();
  });

  it("keeps the final cleanup report readable after a transport disconnect", async () => {
    api.get.mockResolvedValue({ ...report, status: "running", finishedAt: null });
    const mounted = render(view()); await open();
    const uncertain: TemporaryProbeReport = {
      ...report, kind: "sftp", status: "cancelled", cleanup: "unverified",
      probeFile: `/tmp/unfour-a4-${runId}`,
      checks: [{ id: "sftp-cleanup", status: "inconclusive", code: "SFTP_CLEANUP_FAILED", expectedExitCode: null, actualExitCode: null }],
    };
    api.get.mockResolvedValue(uncertain);
    mounted.rerender(view("workspace-a", "handle-a", false));
    await screen.findByText("Check cancelled");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Copy sanitized report" }));
    await waitFor(() => expect(api.copy).toHaveBeenCalled());
    expect(JSON.parse(api.copy.mock.calls[0][0])).toMatchObject({ reports: expect.arrayContaining([expect.objectContaining({ kind: "sftp", cleanup: "unverified", probeFile: uncertain.probeFile })]) });
    expect(api.start).not.toHaveBeenCalled();
    expect(api.terminalCancel).not.toHaveBeenCalled();
  });

  it("does not log raw backend errors or allow an unsafe retry after a failed status read", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => undefined);
    api.get.mockRejectedValueOnce(new Error("secret-host secret-user secret-token"));
    render(view()); await open();
    expect(screen.getByRole("alert")).toHaveTextContent("Could not update the capability check");
    expect(screen.getByRole("button", { name: "Run check" })).toBeDisabled();
    expect(JSON.stringify(log.mock.calls)).not.toContain("secret-");
    expect(JSON.stringify(log.mock.calls)).toContain("TEMPORARY_PROBE_ACTION_FAILED");
    fireEvent.click(screen.getByRole("button", { name: "Refresh status" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    log.mockRestore();
  });
});

it("validates allowlisted values as well as field names", () => {
  const contaminated = {
    ...report, runId: "secret-user", startedAt: "secret-host", probeFile: "/home/secret-user/file", cleanup: "secret-token",
    checks: [{ ...report.checks[0], id: "secret-user", code: "secret-host", status: "secret-token", expectedExitCode: "secret-value" }],
  } as unknown as TemporaryProbeReport;
  const contaminatedApp = { ...app, name: "secret-name", version: "secret-version", commit: "secret-commit", channel: "secret-channel", distribution: "secret-distribution" } as unknown as AppInfo;
  const text = serializeTemporaryCapabilityReport([contaminated], contaminatedApp);
  expect(text).not.toContain("secret-");
  expect(JSON.parse(text)).toMatchObject({ reports: [{ runId: null, probeFile: null, checks: [{ id: "unknown-check", code: "UNKNOWN", status: "inconclusive", expectedExitCode: null }] }] });
});

it.each(["", "https://example.test", "user@host", "host/path", "host name", "host\\path", "host?token=value", "host:22", "[host]", "[::1]:22", "::not-ip", "host\u0000"])("rejects unsafe TCP target %s", (host) => {
  expect(temporaryTcpTarget(host, "80")).toBeNull();
});
it.each(["127.0.0.1", "node.internal", "::1", "[2001:db8::1]", "例子.测试"])("accepts a TCP host without sending any request: %s", (host) => {
  expect(temporaryTcpTarget(host, "18291")).toEqual({ host, port: 18291 });
});
it.each(["", "0", "65536", "1.2", "1e3", "-1"])("rejects invalid TCP port %s", (port) => {
  expect(temporaryTcpTarget("host", port)).toBeNull();
});
