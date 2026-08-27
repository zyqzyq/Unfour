#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  releaseChannelEnvironment,
  resolveTauriInvocation,
} from "./release-channel.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const invocation = resolveTauriInvocation(process.argv.slice(2));
const tauriArgs = invocation.args;

// Synchronize generated version consumers before the Tauri CLI reads its
// configuration. `beforeDevCommand`/`beforeBuildCommand` are too late for
// configuration values that the CLI uses while starting the command.
if (tauriArgs[0] === "dev" || tauriArgs[0] === "build") {
  const sync = spawnSync(
    process.execPath,
    [resolve(repoRoot, "scripts/sync-version.mjs")],
    {
      cwd: repoRoot,
      stdio: "inherit",
    },
  );
  if (sync.error) throw sync.error;
  if (sync.status !== 0) process.exit(sync.status ?? 1);
}

const { channel, profile, environment } = releaseChannelEnvironment(
  repoRoot,
  process.env,
  invocation.defaultChannel,
  invocation.forcedChannel,
);
const args = ["--filter", "@unfour/desktop", "tauri", ...tauriArgs];

let command = "pnpm";
let commandArgs = args;
if (process.platform === "win32") {
  const pnpmEntry = process.env.npm_execpath;
  if (!pnpmEntry) {
    throw new Error(
      "On Windows, invoke this launcher through `pnpm tauri` so pnpm exposes its executable entry point",
    );
  }
  const extension = extname(pnpmEntry).toLowerCase();
  if ([".js", ".cjs", ".mjs"].includes(extension)) {
    command = process.execPath;
    commandArgs = [pnpmEntry, ...args];
  } else {
    command = pnpmEntry;
  }
}

console.log(
  `[run-tauri] version ${profile.version} -> channel=${channel}, distribution=${profile.distribution}`,
);
const result = spawnSync(command, commandArgs, {
  cwd: repoRoot,
  stdio: "inherit",
  env: environment,
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
