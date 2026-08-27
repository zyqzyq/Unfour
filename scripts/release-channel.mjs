#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const numericIdentifier = "(?:0|[1-9]\\d*)";
const stableVersionPattern = new RegExp(
  `^${numericIdentifier}\\.${numericIdentifier}\\.${numericIdentifier}$`,
);
const releaseChannels = new Set(["test", "stable"]);
const distributions = Object.freeze({
  standard: Object.freeze({ updaterEnabled: true }),
  "microsoft-store": Object.freeze({ updaterEnabled: false }),
});

const channelProfiles = Object.freeze({
  test: Object.freeze({
    accountApiUrl: "https://test-api.unfour.dev",
    accountWebUrl: "https://test.unfour.dev",
    updaterEndpoint: "https://release.unfour.dev/test/latest.json",
    allowLoopbackHttp: false,
    defaultStorageProfile: "test",
  }),
  stable: Object.freeze({
    accountApiUrl: "https://api.unfour.dev",
    accountWebUrl: "https://unfour.dev",
    updaterEndpoint: "https://release.unfour.dev/stable/latest.json",
    allowLoopbackHttp: false,
    defaultStorageProfile: "stable",
  }),
});

export function readWorkspaceVersion(repoRoot) {
  const cargo = readFileSync(resolve(repoRoot, "Cargo.toml"), "utf8");
  const workspaceBlock = cargo.match(
    /\[workspace\.package\][\s\S]*?(?:\r?\n\[|$)/,
  )?.[0];
  const version = workspaceBlock?.match(
    /^\s*version\s*=\s*"([^"]+)"\s*$/m,
  )?.[1];
  if (!version) throw new Error("Missing [workspace.package].version");
  return version;
}

export function resolveReleaseChannel(
  explicitChannel,
  defaultChannel = "test",
) {
  const channel = explicitChannel || defaultChannel;
  if (!releaseChannels.has(channel)) {
    throw new Error(
      `UNFOUR_RELEASE_CHANNEL must be exactly "test" or "stable", got ${JSON.stringify(explicitChannel)}`,
    );
  }
  return channel;
}

export function resolveDistribution(explicitDistribution) {
  const distribution = explicitDistribution || "standard";
  if (!Object.hasOwn(distributions, distribution)) {
    throw new Error(
      `UNFOUR_DISTRIBUTION must be exactly "standard" or "microsoft-store", got ${JSON.stringify(explicitDistribution)}`,
    );
  }
  return distribution;
}

export function resolveBuildProfile(
  version,
  explicitChannel,
  explicitDistribution,
  defaultChannel = "test",
) {
  if (!stableVersionPattern.test(version)) {
    throw new Error(
      `Unfour release versions must use X.Y.Z, got ${JSON.stringify(version)}`,
    );
  }
  const releaseChannel = resolveReleaseChannel(
    explicitChannel,
    defaultChannel,
  );
  const distribution = resolveDistribution(explicitDistribution);
  const definition = channelProfiles[releaseChannel];
  const updaterEnabled = distributions[distribution].updaterEnabled;
  return Object.freeze({
    version,
    tag: `v${version}`,
    kind: releaseChannel,
    releaseChannel,
    prerelease: false,
    ...definition,
    distribution,
    updaterEnabled,
    updaterEndpoint: updaterEnabled ? definition.updaterEndpoint : null,
  });
}

export function resolveTauriInvocation(args) {
  if (args[0] === "build:test") {
    return {
      args: ["build", ...args.slice(1)],
      defaultChannel: "test",
      forcedChannel: "test",
    };
  }
  return {
    args,
    defaultChannel: args[0] === "build" ? "stable" : "test",
    forcedChannel: undefined,
  };
}

export function releaseChannelEnvironment(
  repoRoot,
  environment = process.env,
  defaultChannel = "test",
  forcedChannel,
) {
  const version = readWorkspaceVersion(repoRoot);
  const profile = resolveBuildProfile(
    version,
    forcedChannel ?? environment.UNFOUR_RELEASE_CHANNEL,
    environment.UNFOUR_DISTRIBUTION,
    defaultChannel,
  );
  return {
    version,
    channel: profile.releaseChannel,
    profile,
    environment: {
      ...environment,
      UNFOUR_RELEASE_CHANNEL: profile.releaseChannel,
      UNFOUR_DISTRIBUTION: profile.distribution,
    },
  };
}

export function tauriEnvironment(
  environment = process.env,
  defaultChannel = "test",
  forcedChannel,
) {
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  return releaseChannelEnvironment(
    repoRoot,
    environment,
    defaultChannel,
    forcedChannel,
  );
}

export function resolveMsixPackageProfile(profile, packageChannel) {
  if (packageChannel !== "Store" && packageChannel !== "Test") {
    throw new Error("PackageChannel must be exactly Store or Test");
  }
  if (packageChannel === "Store" && profile.releaseChannel !== "stable") {
    throw new Error("PackageChannel Store requires a Stable X.Y.Z build");
  }
  if (packageChannel === "Test" && profile.releaseChannel !== "test") {
    throw new Error("PackageChannel Test requires a Test-channel X.Y.Z build");
  }
  if (profile.distribution !== "microsoft-store") {
    throw new Error(
      `MSIX packaging requires distribution=microsoft-store, got ${profile.distribution}`,
    );
  }
  if (profile.updaterEnabled || profile.updaterEndpoint !== null) {
    throw new Error(
      "Microsoft Store builds must disable the standard updater endpoint",
    );
  }

  const core = profile.version.split(".").map(Number);
  for (const segment of [...core, 0]) {
    if (!Number.isSafeInteger(segment) || segment < 0 || segment > 65_535) {
      throw new Error(
        `Version ${profile.version} cannot be represented as an MSIX dot-quad; every segment must be <= 65535`,
      );
    }
  }
  return Object.freeze({
    ...profile,
    packageChannel,
    msixVersion: [...core, 0].join("."),
    artifactLabel: packageChannel === "Store" ? "STORE" : "TEST",
  });
}

function argument(arguments_, name) {
  const index = arguments_.indexOf(name);
  if (index === -1) return undefined;
  const value = arguments_[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printLines(profile) {
  const entries = {
    profile_kind: profile.kind,
    release_channel: profile.releaseChannel,
    distribution: profile.distribution,
    account_api_url: profile.accountApiUrl,
    account_web_url: profile.accountWebUrl,
    updater_enabled: profile.updaterEnabled ? "1" : "0",
    updater_endpoint: profile.updaterEndpoint ?? "",
    allow_loopback_http: profile.allowLoopbackHttp ? "1" : "0",
    default_storage_profile: profile.defaultStorageProfile,
  };
  process.stdout.write(
    `${Object.entries(entries)
      .map(([key, value]) => `${key}=${value}`)
      .join("\n")}\n`,
  );
}

function runCli(arguments_) {
  const allowed = new Set([
    "--version",
    "--expected-channel",
    "--distribution",
    "--package-channel",
    "--format",
  ]);
  for (let index = 0; index < arguments_.length; index += 2) {
    if (!allowed.has(arguments_[index])) {
      throw new Error(`Unknown argument: ${arguments_[index]}`);
    }
  }
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const version =
    argument(arguments_, "--version") ?? readWorkspaceVersion(repoRoot);
  const profile = resolveBuildProfile(
    version,
    argument(arguments_, "--expected-channel"),
    argument(arguments_, "--distribution"),
  );
  const packageChannel = argument(arguments_, "--package-channel");
  const result = packageChannel
    ? resolveMsixPackageProfile(profile, packageChannel)
    : profile;
  const format = argument(arguments_, "--format") ?? "json";
  if (format === "json") {
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } else if (format === "lines" && !packageChannel) {
    printLines(result);
  } else {
    throw new Error(`Unsupported output format ${format}`);
  }
}

const invokedPath = process.argv[1]
  ? pathToFileURL(resolve(process.argv[1])).href
  : null;
if (invokedPath === import.meta.url) runCli(process.argv.slice(2));
