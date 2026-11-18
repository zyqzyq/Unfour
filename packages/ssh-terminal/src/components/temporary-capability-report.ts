import type { AppInfo, TemporaryProbeReport } from "@unfour/command-client";

const codes = new Set([
  "CURL_MISSING", "TERMINAL_TOOLS_MISSING", "CURL_OPTIONS_UNSUPPORTED", "TERMINAL_PROTOCOL_ERROR",
  "OK", "CANCELLED", "TIMEOUT", "CHANNEL_REJECTED", "CHANNEL_BUSY", "EXEC_REJECTED", "EXIT_STATUS_MISSING",
  "EXIT_STATUS_MISMATCH", "OUTPUT_MISMATCH", "OUTPUT_LIMIT", "TRANSPORT_CLOSED", "SFTP_UNAVAILABLE",
  "SFTP_CREATE_FAILED", "SFTP_WRITE_FAILED", "SFTP_READ_FAILED", "SFTP_MISMATCH", "SFTP_CLEANUP_FAILED", "SFTP_PROTOCOL_ERROR",
  "SSH_TUNNEL_TIMEOUT", "SSH_TUNNEL_FORWARD_BUSY", "SSH_TUNNEL_FORWARD_PROHIBITED", "SSH_TUNNEL_TARGET_CONNECT_FAILED",
  "SSH_TUNNEL_CHANNEL_UNSUPPORTED", "SSH_TUNNEL_RESOURCE_SHORTAGE", "SSH_TUNNEL_TRANSPORT_CLOSED", "SSH_TUNNEL_FORWARD_FAILED",
  "TERMINAL_RECOVERY_REQUIRED", "TERMINAL_NOT_READY", "SSH_TASK_TERMINAL_BUSY", "SSH_TASK_TERMINAL_NOT_READY", "SSH_TASK_TERMINAL_OUTPUT_OVERFLOW",
  "SSH_TASK_INTERACTIVE_LOGIN_PROMPT", "INTERACTIVE_LOGIN_PROMPT",
  "SSH_TERMINAL_STARTUP_TIMEOUT", "SSH_TERMINAL_READY_TIMEOUT", "SSH_TERMINAL_ACCEPT_TIMEOUT",
  "SSH_TERMINAL_INPUT_ACK_TIMEOUT", "SSH_TERMINAL_BEGIN_TIMEOUT", "SSH_TERMINAL_END_TIMEOUT",
  "SSH_TERMINAL_RESTORE_TIMEOUT", "SSH_TERMINAL_TRANSPORT_CLOSED",
]);
const checkIds = new Set(["transport-roundtrip", "transport-recovery", "curl-tools", "curl-roundtrip", "curl-recovery", "exec-success", "exec-nonzero", "terminal-success", "terminal-nonzero", "terminal-recovery", "concurrent-channels", "sftp-roundtrip", "sftp-cleanup", "tcp-forward"]);
const statuses = new Set(["pass", "fail", "unsupported", "inconclusive", "cancelled"]);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const timestamp = (value: string | null) => value && /^\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2})$/.test(value) && Number.isFinite(Date.parse(value)) ? value : null;
const exitCode = (value: number | null) => typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff ? value : null;

/** Explicit allowlist at the copy/display boundary, even if a future DTO grows. */
export function sanitizeTemporaryProbeReport(report: TemporaryProbeReport) {
  return {
    runId: uuid.test(report.runId) ? report.runId : null,
    kind: (["exec", "terminal", "transport", "curl", "channels", "sftp", "tcp"] as string[]).includes(report.kind) ? report.kind : null,
    targetIdentityVerified: false,
    status: (["running", "completed", "cancelled"] as string[]).includes(report.status) ? report.status : "completed",
    startedAt: timestamp(report.startedAt),
    finishedAt: timestamp(report.finishedAt),
    checks: report.checks.slice(0, 8).map((check) => ({
      id: checkIds.has(check.id) ? check.id : "unknown-check",
      status: statuses.has(check.status) ? check.status : "inconclusive",
      code: codes.has(check.code) ? check.code : "UNKNOWN",
      expectedExitCode: exitCode(check.expectedExitCode),
      actualExitCode: exitCode(check.actualExitCode),
    })),
    probeFile: report.probeFile && /^\/tmp\/unfour-a4-[0-9a-f-]+$/.test(report.probeFile) && uuid.test(report.probeFile.slice("/tmp/unfour-a4-".length)) ? report.probeFile : null,
    cleanup: report.cleanup && ["removed", "not-created", "unverified"].includes(report.cleanup) ? report.cleanup : null,
  };
}

export function serializeTemporaryCapabilityReport(reports: TemporaryProbeReport[], app: AppInfo | null) {
  return JSON.stringify({
    schemaVersion: 1,
    scope: "temporary-ssh",
    app: {
      version: app && /^\d+\.\d+\.\d+(?:-[a-z0-9.-]+)?(?:\+[a-z0-9.-]+)?$/i.test(app.version) ? app.version : null,
      commit: app?.commit && /^[0-9a-f]{7,40}(?:-dirty)?$/i.test(app.commit) ? app.commit : null,
      channel: app && ["test", "stable"].includes(app.channel) ? app.channel : null,
      distribution: app && ["standard", "microsoft-store"].includes(app.distribution) ? app.distribution : null,
    },
    reports: reports.map(sanitizeTemporaryProbeReport),
    manualChecks: ["second-login", "expiry", "standalone-file-transfer", "helper-exit", "controlled-disconnect", "task-flow-composition"],
  }, null, 2);
}

/** Accept a host and port, never a URL, credentials or HTTP payload. */
export function temporaryTcpTarget(host: string, port: string): { host: string; port: number } | null {
  const value = host.trim();
  if (!value || value.length > 255 || /[\s/@\\?#%]/.test(value) || [...value].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)) return null;
  if ([":", "[", "]"].some((character) => value.includes(character))) {
    try {
      // URL's parser validates IPv6 literals locally; no request is sent.
      const parsed = new URL(`http://${value.startsWith("[") ? value : `[${value}]`}/`);
      if (!parsed.hostname.startsWith("[") || parsed.port) return null;
    } catch { return null; }
  } else if (!/^[\p{L}\p{M}\p{N}_.-]+$/u.test(value)) return null;
  if (!/^\d+$/.test(port)) return null;
  const number = Number(port);
  return Number.isInteger(number) && number > 0 && number <= 65535 ? { host: value, port: number } : null;
}
