import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import { finalizeStandardRelease } from "./finalize-standard-release.mjs";

test("staged Standard installers feed checksums and updater metadata", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  try {
    const installer = "Unfour_1.2.3_windows_x64.exe";
    const linuxInstaller = "Unfour_1.2.3_linux_x64.AppImage";
    writeFileSync(resolve(directory, installer), "same-installer-bytes");
    writeFileSync(resolve(directory, `${installer}.sig`), "signed-fixture\n");
    writeFileSync(resolve(directory, linuxInstaller), "same-linux-appimage-bytes");
    writeFileSync(resolve(directory, `${linuxInstaller}.sig`), "signed-linux-fixture\n");
    finalizeStandardRelease({
      assetsDir: directory,
      version: "1.2.3",
      baseUrl: "https://release.unfour.dev/",
      notes: "Fixture",
    });
    const latest = JSON.parse(readFileSync(resolve(directory, "latest.json"), "utf8"));
    assert.equal(latest.version, "1.2.3");
    assert.equal(latest.platforms["windows-x86_64"].signature, "signed-fixture");
    assert.deepEqual(Object.keys(latest.platforms).sort(), ["linux-x86_64", "windows-x86_64"]);
    assert.equal(latest.platforms["linux-x86_64"].signature, "signed-linux-fixture");
    assert.equal(
      latest.platforms["linux-x86_64"].url,
      `https://release.unfour.dev/stable/1.2.3/${linuxInstaller}`,
    );
    assert.equal(
      latest.platforms["windows-x86_64"].url,
      `https://release.unfour.dev/stable/1.2.3/${installer}`,
    );
    const checksums = readFileSync(resolve(directory, "SHA256SUMS.txt"), "utf8");
    assert.match(checksums, new RegExp(`  ${installer}\\n`));
    assert.match(checksums, new RegExp(`  ${installer}\\.sig\\n`));
    assert.match(checksums, new RegExp(`  ${linuxInstaller}\\n`));
    assert.match(checksums, new RegExp(`  ${linuxInstaller}\\.sig\\n`));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("requires a signed Linux AppImage in the Standard release", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  try {
    const installer = "Unfour_1.2.3_windows_x64.exe";
    writeFileSync(resolve(directory, installer), "windows-bytes");
    writeFileSync(resolve(directory, `${installer}.sig`), "windows-signature\n");
    writeFileSync(resolve(directory, "Unfour_1.2.3_linux_x64.AppImage"), "linux-bytes");
    assert.throws(
      () =>
        finalizeStandardRelease({
          assetsDir: directory,
          version: "1.2.3",
          baseUrl: "https://release.unfour.dev",
        }),
      /missing .*\.sig/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects non-canonical Linux deb and rpm package assets", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  try {
    writeFileSync(resolve(directory, "Unfour_1.2.3_windows_x64.exe"), "windows-bytes");
    writeFileSync(resolve(directory, "Unfour_1.2.3_windows_x64.exe.sig"), "windows-signature\n");
    writeFileSync(resolve(directory, "Unfour_1.2.3_linux_x64.AppImage"), "linux-bytes");
    writeFileSync(resolve(directory, "Unfour_1.2.3_linux_x64.AppImage.sig"), "linux-signature\n");
    writeFileSync(resolve(directory, "Unfour_1.2.3_linux_x64.deb"), "deb-bytes");
    writeFileSync(resolve(directory, "Unfour_1.2.3_linux_x64.rpm"), "rpm-bytes");
    assert.throws(
      () =>
        finalizeStandardRelease({
          assetsDir: directory,
          version: "1.2.3",
          baseUrl: "https://release.unfour.dev",
        }),
      /Non-canonical Linux package assets must not be staged/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects an unsigned updater installer", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  try {
    writeFileSync(resolve(directory, "Unfour_1.2.3_windows_x64.exe"), "bytes");
    assert.throws(
      () =>
        finalizeStandardRelease({
          assetsDir: directory,
          version: "1.2.3",
          baseUrl: "https://release.unfour.dev",
        }),
      /missing .*\.sig/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects a Standard release without a Linux AppImage", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  try {
    const installer = "Unfour_1.2.3_windows_x64.exe";
    writeFileSync(resolve(directory, installer), "windows-bytes");
    writeFileSync(resolve(directory, `${installer}.sig`), "windows-signature\n");
    assert.throws(
      () =>
        finalizeStandardRelease({
          assetsDir: directory,
          version: "1.2.3",
          baseUrl: "https://release.unfour.dev",
        }),
      /Standard Linux updater artifact and signature are required/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("Standard workflow builds once and republishes the same staged files", () => {
  const workflow = readFileSync(
    new URL("../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );
  assert.equal(workflow.match(/pnpm run tauri build/g)?.length, 1);
  assert.match(workflow, /pattern: release-assets-\*/);
  assert.match(workflow, /linux_x64\.AppImage/);
  assert.match(workflow, /linux_x64\.AppImage\.sig/);
  assert.doesNotMatch(workflow, /bundle\/(?:deb|rpm)/);
  assert.doesNotMatch(workflow, /linux_x64\.(?:deb|rpm)/);
  assert.match(workflow, /aws s3 cp release-assets[\s\S]*sha256sum -c/);
  assert.match(workflow, /sha256sum -c[\s\S]*softprops\/action-gh-release/);
  assert.match(workflow, /files: release-assets\/\*/);
  assert.match(workflow, /Refusing to overwrite immutable R2 object/);
});
