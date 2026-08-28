import { describe, expect, it } from "vitest";
import {
  APP_GITHUB_URL,
  APP_VERSION,
  APP_WEBSITE_URL,
  createVersionInfo,
  formatShortCommit,
} from "./settings-config";

describe("settings config", () => {
  it("uses centralized product metadata and links", () => {
    expect(APP_VERSION).toMatch(/^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$/);
    expect(APP_WEBSITE_URL).toBe("https://unfour.dev/");
    expect(APP_GITHUB_URL).toBe("https://github.com/zyqzyq/Unfour");
  });

  it("formats copyable version details for support reports", () => {
    expect(
      createVersionInfo({
        platform: "Win32",
        userAgent: "Vitest",
      }),
    ).toContain(`Unfour ${APP_VERSION}`);
    expect(createVersionInfo({ platform: "Win32", userAgent: "Vitest" })).toContain(
      "Platform: Win32",
    );
  });

  it("includes the unified version, distribution, channel, and commit", () => {
    const info = createVersionInfo(
      { platform: "Win32", userAgent: "Vitest" },
      {
        name: "Unfour",
        version: "0.1.0",
        distribution: "standard",
        channel: "test",
        commit: "0123456789abcdef",
      },
    );
    expect(info).toContain("Unfour 0.1.0");
    expect(info).toContain("Distribution: standard");
    expect(info).toContain("Channel: test");
    expect(info).toContain("Commit: 0123456789abcdef");
  });

  it("shortens commits to 12 chars and preserves the dirty marker", () => {
    expect(formatShortCommit("0123456789abcdef0123456789abcdef")).toBe("0123456789ab");
    expect(formatShortCommit("0123456789abcdef-dirty")).toBe("0123456789ab-dirty");
    expect(formatShortCommit(null)).toBe("");
    expect(formatShortCommit(undefined)).toBe("");
  });
});
