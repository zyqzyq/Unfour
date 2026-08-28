#!/usr/bin/env node

import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const DEFAULT_LATEST_URL =
  "https://releases.unfour.dev/stable/latest.json";

const stableVersionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function parseSemVer(version) {
  if (typeof version !== "string" || !stableVersionPattern.test(version)) {
    throw new Error(
      `Stable release version must be X.Y.Z, got ${JSON.stringify(version)}`,
    );
  }
  return version.split(".").map((segment) => BigInt(segment));
}

export function compareSemVer(candidateVersion, currentVersion) {
  const candidate = parseSemVer(candidateVersion);
  const current = parseSemVer(currentVersion);
  for (let index = 0; index < candidate.length; index += 1) {
    if (candidate[index] > current[index]) return 1;
    if (candidate[index] < current[index]) return -1;
  }
  return 0;
}

export function evaluateUpdateOrder(candidateVersion, currentVersion) {
  const comparison = compareSemVer(candidateVersion, currentVersion);
  return {
    allowed: comparison >= 0,
    candidateVersion,
    currentVersion,
    relation: comparison > 0 ? "newer" : comparison < 0 ? "older" : "equal",
  };
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

export async function checkUpdateOrder({
  candidateVersion,
  latestUrl = DEFAULT_LATEST_URL,
  fetchImpl = globalThis.fetch,
}) {
  parseSemVer(candidateVersion);
  if (typeof fetchImpl !== "function") {
    throw new Error("Node fetch is required to read stable/latest.json");
  }

  let response;
  try {
    response = await fetchImpl(latestUrl, {
      headers: { accept: "application/json" },
    });
  } catch (error) {
    throw new Error(
      `Failed to read current stable/latest.json: ${errorMessage(error)}`,
    );
  }

  if (response.status === 404) {
    return {
      allowed: true,
      candidateVersion,
      currentVersion: null,
      relation: "missing",
    };
  }
  if (!response.ok) {
    throw new Error(
      `Failed to read current stable/latest.json: HTTP ${response.status}`,
    );
  }

  let latest;
  try {
    latest = await response.json();
  } catch (error) {
    throw new Error(
      `Failed to parse current stable/latest.json: ${errorMessage(error)}`,
    );
  }
  if (!latest || typeof latest !== "object" || Array.isArray(latest)) {
    throw new Error("Current stable/latest.json must contain a JSON object");
  }

  const decision = evaluateUpdateOrder(candidateVersion, latest.version);
  if (!decision.allowed) {
    throw new Error(
      `Refusing to publish stable/latest.json: candidate ${candidateVersion} is older than current ${latest.version}`,
    );
  }
  return decision;
}

function argument(arguments_, name) {
  const index = arguments_.indexOf(name);
  if (index === -1) return undefined;
  const value = arguments_[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a value`);
  return value;
}

async function run(arguments_) {
  const candidateVersion = argument(arguments_, "--candidate-version");
  if (!candidateVersion) throw new Error("--candidate-version is required");
  const latestUrl = argument(arguments_, "--latest-url") ?? DEFAULT_LATEST_URL;
  const decision = await checkUpdateOrder({ candidateVersion, latestUrl });
  process.stdout.write(
    `[update-order] stable/latest.json promotion allowed: candidate=${decision.candidateVersion} relation=${decision.relation} current=${decision.currentVersion ?? "none"}\n`,
  );
}

const invokedPath = process.argv[1]
  ? pathToFileURL(resolve(process.argv[1])).href
  : null;
if (invokedPath === import.meta.url) {
  run(process.argv.slice(2)).catch((error) => {
    console.error(`[update-order] ${error.message}`);
    process.exitCode = 1;
  });
}
