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

export type VersionInfoApp = {
  name: string;
  version: string;
  distribution?: string;
  channel?: string;
  commit?: string | null;
};

export function createVersionInfo(
  environment = getVersionEnvironment(),
  app: VersionInfoApp = {
    name: APP_NAME,
    version: APP_VERSION,
  },
) {
  // Support reports need the complete unified identity. Fields that were not
  // supplied are omitted rather than printed as "undefined".
  return [
    `${app.name} ${app.version}`,
    ...(app.distribution ? [`Distribution: ${app.distribution}`] : []),
    ...(app.channel ? [`Channel: ${app.channel}`] : []),
    ...(app.commit ? [`Commit: ${app.commit}`] : []),
    `Platform: ${environment.platform}`,
    `User agent: ${environment.userAgent}`,
    `Website: ${APP_WEBSITE_URL}`,
    `GitHub: ${APP_GITHUB_URL}`,
  ].join("\n");
}

// Format the commit for display: keep up to 12 leading hex chars and preserve
// the `-dirty` marker that build.rs appends for modified working trees.
export function formatShortCommit(commit: string | null | undefined): string {
  if (!commit) {
    return "";
  }
  const dirtySuffix = commit.endsWith("-dirty") ? "-dirty" : "";
  const base = dirtySuffix ? commit.slice(0, commit.length - dirtySuffix.length) : commit;
  const short = base.slice(0, 12);
  return `${short}${dirtySuffix}`;
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
