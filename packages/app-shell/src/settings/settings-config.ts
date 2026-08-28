import rootPackage from "../../../../package.json";

export const APP_NAME = "Unfour";
export const APP_VERSION = rootPackage.version;
export const APP_WEBSITE_URL = "https://unfour.dev/";
export const APP_GITHUB_URL = "https://github.com/zyqzyq/Unfour";

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

function getVersionEnvironment() {
  return {
    platform: globalThis.navigator?.platform ?? "unknown",
    userAgent: globalThis.navigator?.userAgent ?? "unknown",
  };
}
