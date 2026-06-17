import { describe, expect, it } from "vitest";
import { formatTerminalError } from "./errors";

describe("formatTerminalError", () => {
  it("formats authentication failures", () => {
    expect(formatTerminalError(new Error("ssh authentication failed"))).toContain(
      "SSH authentication failed",
    );
  });

  it("formats network timeout failures", () => {
    expect(
      formatTerminalError("ssh connection to host:22 timed out after 10s"),
    ).toContain("timed out");
  });

  it("formats unreachable host failures", () => {
    expect(formatTerminalError("No route to host")).toContain("unreachable");
  });

  it("redacts sensitive fallback errors", () => {
    expect(formatTerminalError("password=secret")).toBe("<redacted>");
  });
});
