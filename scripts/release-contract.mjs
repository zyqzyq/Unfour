#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readWorkspaceVersion, resolveBuildProfile } from "./release-channel.mjs";

export function validateStandardStableRelease(version, tag) {
  const profile = resolveBuildProfile(version, "stable", "standard");
  if (tag !== profile.tag) {
    throw new Error(
      `Standard Stable tag must exactly match ${profile.tag}, got ${JSON.stringify(tag)}`,
    );
  }
  if (!profile.updaterEnabled || !profile.updaterEndpoint) {
    throw new Error("Standard Stable must enable the internal updater endpoint");
  }
  return {
    version,
    tag,
    channel: profile.releaseChannel,
    distribution: profile.distribution,
    updaterEndpoint: profile.updaterEndpoint,
    prerelease: false,
  };
}

export function resolveStandardStableRelease(repoRoot, tag) {
  return validateStandardStableRelease(readWorkspaceVersion(repoRoot), tag);
}

export function validateTauriUpdaterConfigs(
  tauri,
  release,
  publicKeyFile,
) {
  const publicKey = publicKeyFile.replace(/\r?\n$/, "");
  const updater = tauri.plugins?.updater;
  if (!updater || typeof updater !== "object" || Array.isArray(updater)) {
    throw new Error("base tauri.conf.json must define plugins.updater");
  }
  if (updater.active !== true) {
    throw new Error("base plugins.updater.active must be true");
  }
  if (!publicKey || updater.pubkey !== publicKey) {
    throw new Error(
      "base updater public key must exactly match updater_secret.key.pub",
    );
  }
  if (updater.windows?.installMode !== "passive") {
    throw new Error("base updater Windows installMode must remain passive");
  }
  if (
    tauri.bundle?.createUpdaterArtifacts === true ||
    tauri.bundle?.createUpdaterArtifacts === "v1Compatible"
  ) {
    throw new Error("base tauri.conf.json must not create updater artifacts");
  }

  if (release.plugins?.updater !== undefined) {
    throw new Error(
      "tauri.release.conf.json must not duplicate plugins.updater runtime config",
    );
  }
  const unexpectedTopLevelKeys = Object.keys(release).filter(
    (key) => key !== "$schema" && key !== "bundle",
  );
  const unexpectedBundleKeys = Object.keys(release.bundle ?? {}).filter(
    (key) => key !== "createUpdaterArtifacts",
  );
  if (unexpectedTopLevelKeys.length > 0 || unexpectedBundleKeys.length > 0) {
    throw new Error(
      "tauri.release.conf.json may only configure bundle.createUpdaterArtifacts",
    );
  }
  if (release.bundle?.createUpdaterArtifacts !== true) {
    throw new Error(
      "tauri.release.conf.json must set bundle.createUpdaterArtifacts=true",
    );
  }
}

export function validateTauriUpdaterConfigFiles(repoRoot) {
  const tauriRoot = resolve(repoRoot, "apps/desktop/src-tauri");
  const tauri = JSON.parse(
    readFileSync(resolve(tauriRoot, "tauri.conf.json"), "utf8"),
  );
  const release = JSON.parse(
    readFileSync(resolve(tauriRoot, "tauri.release.conf.json"), "utf8"),
  );
  const publicKeyFile = readFileSync(
    resolve(tauriRoot, "updater_secret.key.pub"),
    "utf8",
  );
  validateTauriUpdaterConfigs(tauri, release, publicKeyFile);
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : undefined;
if (invokedPath === fileURLToPath(import.meta.url)) {
  try {
    const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
    const result = resolveStandardStableRelease(repoRoot, process.argv[2]);
    validateTauriUpdaterConfigFiles(repoRoot);
    process.stdout.write(
      `version=${result.version}\ntag=${result.tag}\nchannel=${result.channel}\ndistribution=${result.distribution}\nupdater_endpoint=${result.updaterEndpoint}\nprerelease=${result.prerelease}\n`,
    );
  } catch (error) {
    console.error(`[release-contract] ${error.message}`);
    process.exitCode = 1;
  }
}
