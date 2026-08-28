#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

function argument(arguments_, name) {
  const index = arguments_.indexOf(name);
  if (index === -1) return undefined;
  const value = arguments_[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a value`);
  return value;
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function updaterPlatform(files, assetsDir, name, platform, baseUrl, version) {
  if (!files.includes(name)) return undefined;
  const signatureName = `${name}.sig`;
  if (!files.includes(signatureName)) {
    throw new Error(`Updater artifact ${name} is missing ${signatureName}`);
  }
  const signature = readFileSync(resolve(assetsDir, signatureName), "utf8").trim();
  if (!signature) throw new Error(`Updater signature ${signatureName} is empty`);
  return [platform, { signature, url: `${baseUrl}/stable/${version}/${name}` }];
}

export function finalizeStandardRelease({ assetsDir, version, baseUrl, notes = "" }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Standard release version must be X.Y.Z, got ${JSON.stringify(version)}`);
  }
  const normalizedBaseUrl = baseUrl.replace(/\/+$/, "");
  const files = readdirSync(assetsDir)
    .filter((name) => name !== "SHA256SUMS.txt" && name !== "latest.json")
    .sort();
  const windowsInstaller = `Unfour_${version}_windows_x64.exe`;
  if (!files.includes(windowsInstaller)) {
    throw new Error(`Standard release is missing ${windowsInstaller}`);
  }

  const candidates = [
    [windowsInstaller, "windows-x86_64"],
    [`Unfour_${version}_macos_arm64.app.tar.gz`, "darwin-aarch64"],
    [`Unfour_${version}_macos_x64.app.tar.gz`, "darwin-x86_64"],
    [`Unfour_${version}_linux_x64.AppImage`, "linux-x86_64"],
  ];
  const platforms = Object.fromEntries(
    candidates
      .map(([name, platform]) =>
        updaterPlatform(files, assetsDir, name, platform, normalizedBaseUrl, version),
      )
      .filter(Boolean),
  );
  if (!platforms["windows-x86_64"]) {
    throw new Error("Standard Windows updater artifact and signature are required");
  }

  const checksums = files.map((name) => `${sha256(resolve(assetsDir, name))}  ${name}`);
  writeFileSync(resolve(assetsDir, "SHA256SUMS.txt"), `${checksums.join("\n")}\n`);
  writeFileSync(
    resolve(assetsDir, "latest.json"),
    `${JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2)}\n`,
  );
  return { files, platforms };
}

function run(arguments_) {
  const assetsDir = resolve(argument(arguments_, "--assets-dir") ?? "release-assets");
  const version = argument(arguments_, "--version");
  if (!version) throw new Error("--version is required");
  const baseUrl = argument(arguments_, "--base-url") ?? "https://release.unfour.dev";
  const notesPath = argument(arguments_, "--notes-file");
  const notes = notesPath ? readFileSync(resolve(notesPath), "utf8").trim() : "";
  const result = finalizeStandardRelease({ assetsDir, version, baseUrl, notes });
  process.stdout.write(
    `[standard-release] finalized ${result.files.length} immutable files for ${Object.keys(result.platforms).join(", ")}\n`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    run(process.argv.slice(2));
  } catch (error) {
    console.error(`[standard-release] ${error.message}`);
    process.exitCode = 1;
  }
}
