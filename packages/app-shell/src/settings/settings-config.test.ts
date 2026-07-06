import { describe, expect, it } from "vitest";
import {
  APP_GITHUB_URL,
  APP_VERSION,
  APP_WEBSITE_URL,
  MCP_DOCS_PATH,
  createMcpClientConfig,
  createVersionInfo,
  getMcpCommand,
} from "./settings-config";

describe("settings config", () => {
  it("uses centralized product metadata and links", () => {
    expect(APP_VERSION).toBe("0.1.0");
    expect(APP_WEBSITE_URL).toBe("https://unfour.dev/");
    expect(APP_GITHUB_URL).toBe("https://github.com/zyqzyq/Unfour");
    expect(MCP_DOCS_PATH).toBe("docs/mcp/codex-setup.md");
  });

  it("builds the documented dev MCP command and minimal client config", () => {
    const command = getMcpCommand("win32");
    expect(command).toBe("D:\\Program Files\\Unfour\\unfour-mcp.exe");

    expect(createMcpClientConfig(command)).toEqual({
      mcpServers: {
        unfour: {
          command,
          args: [],
        },
      },
    });
  });

  it("formats copyable version details for support reports", () => {
    expect(
      createVersionInfo({
        platform: "Win32",
        userAgent: "Vitest",
      }),
    ).toContain("Unfour 0.1.0");
    expect(createVersionInfo({ platform: "Win32", userAgent: "Vitest" })).toContain(
      "Platform: Win32",
    );
  });
});
