#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseSemVer } from "./check-update-order.mjs";

const generatedMetadata = new Set(["SHA256SUMS.txt", "latest.json", "downloads.json"]);

function normalizeBaseUrl(baseUrl) {
  let url;
  try {
    url = new URL(baseUrl);
  } catch {
    throw new Error("Release base URL must be an absolute HTTP(S) URL");
  }
  if (
    typeof baseUrl !== "string" ||
    !/^https?:\/\//i.test(baseUrl) ||
    !["https:", "http:"].includes(url.protocol) ||
    url.username || url.password || /[?#\\\s]/.test(baseUrl)
  ) {
    throw new Error("Release base URL must use HTTP(S) without credentials, query, fragment, or whitespace");
  }
  return url.href.replace(/\/+$/, "");
}

// files is the inventory of regular files discovered in release-assets.
export function createDownloadsManifest({ files, version, baseUrl }) {
  parseSemVer(version);
  const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
  const installers = [
    ["windows-x64", `Unfour_${version}_windows_x64.exe`],
    ["macos-arm64", `Unfour_${version}_macos_arm64.dmg`],
    ["macos-x64", `Unfour_${version}_macos_x64.dmg`],
    ["linux-x64", `Unfour_${version}_linux_x64.AppImage`],
  ];
  const downloads = Object.fromEntries(installers.map(([platform, name]) => {
    if (!files.includes(name)) {
      throw new Error(`Standard release is missing required installer ${name}`);
    }
    return [platform, { url: `${normalizedBaseUrl}/stable/${version}/${name}` }];
  }));
  return { version, downloads };
}

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
  if (!files.includes(name)) {
    throw new Error(`Standard release is missing required updater artifact ${name}`);
  }
  const signatureName = `${name}.sig`;
  if (!files.includes(signatureName)) {
    throw new Error(`Updater artifact ${name} is missing ${signatureName}`);
  }
  const signature = readFileSync(resolve(assetsDir, signatureName), "utf8").trim();
  if (!signature) throw new Error(`Updater signature ${signatureName} is empty`);
  return [platform, { signature, url: `${baseUrl}/stable/${version}/${name}` }];
}

export function finalizeStandardRelease({ assetsDir, version, baseUrl, notes = "" }) {
  parseSemVer(version);
  const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
  const files = readdirSync(assetsDir, { withFileTypes: true })
    .filter((entry) => !generatedMetadata.has(entry.name))
    .map((entry) => {
      if (!entry.isFile()) {
        throw new Error(`Standard release asset must be a regular file: ${entry.name}`);
      }
      return entry.name;
    })
    .sort();
  const nonCanonicalLinuxPackages = files.filter((name) => /\.(?:deb|rpm)$/i.test(name));
  if (nonCanonicalLinuxPackages.length > 0) {
    throw new Error(
      `Non-canonical Linux package assets must not be staged: ${nonCanonicalLinuxPackages.join(", ")}`,
    );
  }
  const downloadsManifest = createDownloadsManifest({ files, version, baseUrl: normalizedBaseUrl });

  const candidates = [
    [`Unfour_${version}_windows_x64.exe`, "windows-x86_64"],
    [`Unfour_${version}_macos_arm64.app.tar.gz`, "darwin-aarch64"],
    [`Unfour_${version}_macos_x64.app.tar.gz`, "darwin-x86_64"],
    [`Unfour_${version}_linux_x64.AppImage`, "linux-x86_64"],
  ];
  const platforms = Object.fromEntries(
    candidates.map(([name, platform]) =>
      updaterPlatform(files, assetsDir, name, platform, normalizedBaseUrl, version),
    ),
  );

  const checksums = files.map((name) => `${sha256(resolve(assetsDir, name))}  ${name}`);
  writeFileSync(resolve(assetsDir, "SHA256SUMS.txt"), `${checksums.join("\n")}\n`);
  writeFileSync(
    resolve(assetsDir, "latest.json"),
    `${JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2)}\n`,
  );
  writeFileSync(
    resolve(assetsDir, "downloads.json"),
    `${JSON.stringify(downloadsManifest, null, 2)}\n`,
  );
  return { files, platforms, downloads: downloadsManifest.downloads };
}

function run(arguments_) {
  const assetsDir = resolve(argument(arguments_, "--assets-dir") ?? "release-assets");
  const version = argument(arguments_, "--version");
  if (!version) throw new Error("--version is required");
  const baseUrl = argument(arguments_, "--base-url") ?? "https://releases.unfour.dev";
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
