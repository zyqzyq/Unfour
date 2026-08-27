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

function validateTrackedUpdaterPublicKey(repoRoot) {
  const tauri = JSON.parse(
    readFileSync(resolve(repoRoot, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
  );
  const updater = JSON.parse(
    readFileSync(
      resolve(repoRoot, "apps/desktop/src-tauri/tauri.updater.conf.json"),
      "utf8",
    ),
  );
  const publicKeyFile = readFileSync(
    resolve(repoRoot, "apps/desktop/src-tauri/updater_secret.key.pub"),
    "utf8",
  );
  const publicKey = publicKeyFile.trimEnd();
  if (tauri.bundle?.createUpdaterArtifacts === true || tauri.plugins?.updater?.pubkey) {
    throw new Error("base tauri.conf.json must remain updater-signing-key free");
  }
  if (updater.bundle?.createUpdaterArtifacts !== true) {
    throw new Error("tauri.updater.conf.json must create updater artifacts");
  }
  if (!publicKey || updater.plugins?.updater?.pubkey !== publicKey) {
    throw new Error("tracked updater config must exactly match updater_secret.key.pub");
  }
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : undefined;
if (invokedPath === fileURLToPath(import.meta.url)) {
  try {
    const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
    const result = resolveStandardStableRelease(repoRoot, process.argv[2]);
    validateTrackedUpdaterPublicKey(repoRoot);
    process.stdout.write(
      `version=${result.version}\ntag=${result.tag}\nchannel=${result.channel}\ndistribution=${result.distribution}\nupdater_endpoint=${result.updaterEndpoint}\nprerelease=${result.prerelease}\n`,
    );
  } catch (error) {
    console.error(`[release-contract] ${error.message}`);
    process.exitCode = 1;
  }
}
