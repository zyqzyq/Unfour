import { describe, expect, it } from "vitest";
import { extractErrorDetail } from "./feedback";

describe("extractErrorDetail", () => {
  it("reads Error, string, and Tauri AppError-shaped payloads", () => {
    expect(extractErrorDetail(new Error("validation error: Command step requires a command"))).toBe(
      "Command step requires a command",
    );
    expect(extractErrorDetail("not found: SSH task")).toBe("SSH task");
    expect(
      extractErrorDetail({
        code: "VALIDATION_ERROR",
        message: "validation error: Upload and Download paths cannot be empty",
      }),
    ).toBe("Upload and Download paths cannot be empty");
  });

  it("returns undefined for empty payloads", () => {
    expect(extractErrorDetail(null)).toBeUndefined();
    expect(extractErrorDetail({})).toBeUndefined();
    expect(extractErrorDetail(new Error("   "))).toBeUndefined();
  });
});
