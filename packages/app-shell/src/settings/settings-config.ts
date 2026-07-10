import rootPackage from "../../../../package.json";

export const APP_NAME = "Unfour";
export const APP_VERSION = rootPackage.version;
export const APP_WEBSITE_URL = "https://unfour.dev/";
export const APP_GITHUB_URL = "https://github.com/zyqzyq/Unfour";
export const MCP_DOCS_PATH = "docs/mcp/codex-setup.md";
export const MCP_DOCS_URL = `${APP_GITHUB_URL}/blob/main/${MCP_DOCS_PATH}`;

export type McpClientConfig = {
  mcpServers: {
    unfour: {
      args: string[];
      command: string;
    };
  };
};

export function getMcpCommand(platform = getRuntimePlatform()) {
  const windows = isWindowsPlatform(platform);
  const binaryName = windows ? "unfour-mcp.exe" : "unfour-mcp";
  return windows
    ? `D:\\Program Files\\Unfour\\${binaryName}`
    : `/path/to/Unfour/${binaryName}`;
}

export function createMcpClientConfig(command = getMcpCommand()): McpClientConfig {
  return {
    mcpServers: {
      unfour: {
        command,
        args: [],
      },
    },
  };
}

export function formatMcpClientConfig(command = getMcpCommand()) {
  return JSON.stringify(createMcpClientConfig(command), null, 2);
}

export function createVersionInfo(
  environment = getVersionEnvironment(),
  app: { name: string; version: string; edition: string } = {
    name: APP_NAME,
    version: APP_VERSION,
    edition: "community",
  },
) {
  return [
    `${app.name} ${app.version} (${app.edition})`,
    `Platform: ${environment.platform}`,
    `User agent: ${environment.userAgent}`,
    `Website: ${APP_WEBSITE_URL}`,
    `GitHub: ${APP_GITHUB_URL}`,
  ].join("\n");
}

function getRuntimePlatform() {
  const platform = globalThis.navigator?.platform?.toLowerCase() ?? "";
  if (platform.includes("win")) return "win32";
  if (platform.includes("mac")) return "darwin";
  if (platform.includes("linux")) return "linux";
  return platform || "unknown";
}

function getVersionEnvironment() {
  return {
    platform: globalThis.navigator?.platform ?? "unknown",
    userAgent: globalThis.navigator?.userAgent ?? "unknown",
  };
}

function isWindowsPlatform(platform: string) {
  return platform.toLowerCase().includes("win");
}
