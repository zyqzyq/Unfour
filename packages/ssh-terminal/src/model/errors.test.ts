import { describe, expect, it } from "vitest";
import { createTranslator } from "@unfour/ui";
import { formatTaskHistoryLog, formatTerminalError } from "./errors";
import { redactTerminalLog } from "./terminal-state";

const t = createTranslator("en");

describe("formatTerminalError", () => {
  it("localizes diagnostic history without rewriting ordinary output markers", () => {
    const log = "[2026-11-17T00:00:00Z] diagnostic SSH_TASK_INTERACTIVE_LOGIN_PROMPT\n[2026-11-17T00:00:00Z] stdout SSH_TASK_INTERACTIVE_LOGIN_PROMPT\n";
    expect(formatTaskHistoryLog(log, t)).toBe(`[2026-11-17T00:00:00Z] diagnostic ${t("ssh.tasks.run.errors.SSH_TASK_INTERACTIVE_LOGIN_PROMPT")}\n[2026-11-17T00:00:00Z] stdout SSH_TASK_INTERACTIVE_LOGIN_PROMPT\n`);
    expect(formatTaskHistoryLog("[2026-11-17T00:00:00Z] diagnostic SSH_TASK_TERMINAL_RECOVERY_REQUIRED", t)).toContain(t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
    expect(formatTaskHistoryLog("[2026-11-17T00:00:00Z] diagnostic SSH_TASK_TERMINAL_RECOVERY_REQUIRED_UNKNOWN", t)).toContain("SSH_TASK_TERMINAL_RECOVERY_REQUIRED_UNKNOWN");
  });
  it.each([
    ["HOST KEY verification failed: permission denied", "ssh.errors.hostKeyMismatch"],
    ["permission denied: connection timeout", "ssh.errors.authenticationFailed"],
    ["connection refused: timeout", "ssh.errors.timeout"],
  ])("preserves error guidance precedence for %s", (message, key) => {
    expect(formatTerminalError(message, t)).toBe(t(key));
  });

  it("formats authentication failures", () => {
    expect(formatTerminalError(new Error("ssh authentication failed"), t)).toContain(
      "SSH authentication failed",
    );
  });

  it("formats network timeout failures", () => {
    expect(
      formatTerminalError("ssh connection to host:22 timed out after 10s", t),
    ).toContain("timed out");
  });

  it("formats unreachable host failures", () => {
    expect(formatTerminalError("No route to host", t)).toContain("unreachable");
  });

  it("redacts sensitive fallback errors", () => {
    expect(formatTerminalError("password=secret", t)).toBe("<redacted>");
  });

  it("keeps missing-credential guidance visible after terminal log redaction", () => {
    const message = formatTerminalError(
      "password ssh session requires a stored password",
      t,
    );

    expect(message).toContain("saved SSH credential is missing");
    expect(redactTerminalLog(`Connection failed: ${message}`)).toBe(
      `Connection failed: ${message}`,
    );
  });

  it("keeps authentication guidance visible after terminal log redaction", () => {
    const message = formatTerminalError("ssh authentication failed", t);

    expect(redactTerminalLog(`Connection failed: ${message}`)).toBe(
      `Connection failed: ${message}`,
    );
  });
});
