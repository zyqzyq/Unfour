#!/usr/bin/env node
// Single source of truth: root Cargo.toml [workspace.package].version
// Propagates the version into tauri.conf.json and every package.json so a
// release bump only requires editing one line in the root Cargo.toml.
// Runs automatically via tauri.conf.json beforeDev/beforeBuild commands.

import { readFileSync, writeFileSync, existsSync, readdirSync, lstatSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

function readWorkspaceVersion() {
  const cargo = readFileSync(resolve(root, "Cargo.toml"), "utf8");
  const block = cargo.match(/\[workspace\.package\][\s\S]*?(?:\n\[|$)/);
  if (!block) {
    throw new Error("Missing [workspace.package] in root Cargo.toml");
  }
  const m = block[0].match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);
  if (!m) {
    throw new Error("Missing version in [workspace.package]");
  }
  return m[1];
}

const version = readWorkspaceVersion();

// tauri.conf.json is the installer version.
const targets = [];
const tauriConf = resolve(root, "apps/desktop/src-tauri/tauri.conf.json");
if (existsSync(tauriConf)) targets.push(tauriConf);

// Every package.json, skipping heavy / generated dirs.
const skipDirs = new Set([
  "node_modules",
  "target",
  "dist",
  "coverage",
  "test-results",
  ".git",
]);
function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    let stats;
    try {
      stats = lstatSync(full);
    } catch {
      // Skip entries we cannot stat (e.g. permission errors).
      continue;
    }
    if (stats.isSymbolicLink()) {
      // Never rewrite symlinks (e.g. CLAUDE.md -> AGENTS.md). lstat avoids
      // following dangling links, which would otherwise throw ENOENT on CI.
      continue;
    }
    if (stats.isDirectory()) {
      if (!skipDirs.has(entry)) walk(full);
    } else if (entry === "package.json") {
      targets.push(full);
    }
  }
}

walk(root);

let changed = 0;
for (const file of targets) {
  const raw = readFileSync(file, "utf8");
  const updated = raw.replace(/^(\s*"version":\s*)"[^"]*"/m, `$1"${version}"`);
  if (updated !== raw) {
    writeFileSync(file, updated);
    changed++;
    console.log(`[sync-version] ${relative(root, file)} -> ${version}`);
  }
}

console.log(`[sync-version] version ${version}, ${changed} file(s) updated`);
