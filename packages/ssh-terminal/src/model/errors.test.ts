import { describe, expect, it } from "vitest";
import { createTranslator } from "@unfour/ui";
import { formatTerminalError } from "./errors";
import { redactTerminalLog } from "./terminal-state";

const t = createTranslator("en");

describe("formatTerminalError", () => {
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
