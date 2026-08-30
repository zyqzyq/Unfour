import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import { createDownloadsManifest, finalizeStandardRelease } from "./finalize-standard-release.mjs";

const version = "1.2.3";
const baseUrl = "https://releases.unfour.dev";
const installers = [
  "Unfour_1.2.3_windows_x64.exe",
  "Unfour_1.2.3_macos_arm64.dmg",
  "Unfour_1.2.3_macos_x64.dmg",
  "Unfour_1.2.3_linux_x64.AppImage",
];
const updaterArtifacts = {
  "windows-x86_64": installers[0],
  "darwin-aarch64": "Unfour_1.2.3_macos_arm64.app.tar.gz",
  "darwin-x86_64": "Unfour_1.2.3_macos_x64.app.tar.gz",
  "linux-x86_64": installers[3],
};
const files = [...new Set([...installers, ...Object.values(updaterArtifacts)])]
  .concat(Object.values(updaterArtifacts).map((name) => `${name}.sig`))
  .sort();
const metadata = ["SHA256SUMS.txt", "latest.json", "downloads.json"];

function stagedRelease(t) {
  const directory = mkdtempSync(resolve(tmpdir(), "unfour-standard-release-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of files) {
    writeFileSync(resolve(directory, name), `fixture-${name}\n`);
  }
  return { assetsDir: directory, version, baseUrl };
}

function assertNoMetadata(directory) {
  for (const name of metadata) assert.equal(existsSync(resolve(directory, name)), false);
}

test("public downloads use exact canonical installer URLs for all four targets", () => {
  const manifest = createDownloadsManifest({ files, version, baseUrl });
  assert.deepEqual(manifest, {
    version: "1.2.3",
    downloads: {
      "windows-x64": { url: "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_windows_x64.exe" },
      "macos-arm64": { url: "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_macos_arm64.dmg" },
      "macos-x64": { url: "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_macos_x64.dmg" },
      "linux-x64": { url: "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_linux_x64.AppImage" },
    },
  });
  assert.doesNotMatch(JSON.stringify(manifest), /\.app\.tar\.gz|\.sig|platforms|signature/);
});

for (const installer of installers) {
  test(`requires ${installer}, even when other architectures and updater archives exist`, () => {
    assert.throws(
      () => createDownloadsManifest({ files: files.filter((name) => name !== installer), version, baseUrl }),
      { message: `Standard release is missing required installer ${installer}` },
    );
  });

  test(`finalize fails without writing metadata when ${installer} is missing on disk`, (t) => {
    const options = stagedRelease(t);
    unlinkSync(resolve(options.assetsDir, installer));
    assert.throws(
      () => finalizeStandardRelease(options),
      { message: `Standard release is missing required installer ${installer}` },
    );
    assertNoMetadata(options.assetsDir);
  });
}

test("rejects wrong-version, misnamed, nested, and non-canonical installer filenames", () => {
  for (const installer of installers) {
    for (const replacement of [
      installer.replace(version, "1.2.4"),
      installer.replace("Unfour_", "unfour_"),
      installer.replace("_x64", "_x86_64").replace("_arm64", "_aarch64"),
      `nested/${installer}`,
    ]) {
      assert.throws(
        () => createDownloadsManifest({
          files: files.map((name) => name === installer ? replacement : name), version, baseUrl,
        }),
        { message: `Standard release is missing required installer ${installer}` },
      );
    }
  }
});

test("public downloads require a plain stable SemVer version", () => {
  for (const invalid of [undefined, null, 123, "", "v1.2.3", "1.2", "1.2.3.0", "1.2.3-rc.1", "1.2.3+build", "01.2.3", "1.02.3", "1.2.03", "1.2.3\n", "../1.2.3"]) {
    assert.throws(
      () => createDownloadsManifest({ files, version: invalid, baseUrl }),
      /Stable release version must be X\.Y\.Z/,
    );
  }
});

test("normalizes trailing slashes while preserving a custom base URL path", () => {
  for (const root of [baseUrl, "https://cdn.example.test/releases", "http://localhost:8080/releases"]) {
    for (const suffix of ["", "/", "///"]) {
      const { downloads } = createDownloadsManifest({ files, version, baseUrl: `${root}${suffix}` });
      assert.equal(downloads["macos-arm64"].url, `${root}/stable/1.2.3/Unfour_1.2.3_macos_arm64.dmg`);
    }
  }
});

test("rejects malformed or ambiguous public base URLs", () => {
  for (const invalid of [undefined, "", "/releases", "https:releases.unfour.dev", "ftp://example.test", "https://user:pass@example.test", `${baseUrl}?token=secret`, `${baseUrl}#fragment`, `${baseUrl}?`, `${baseUrl} `, "https:\\example.test"]) {
    assert.throws(
      () => createDownloadsManifest({ files, version, baseUrl: invalid }),
      /Release base URL/,
    );
  }
});

test("staged bytes feed separate downloads and unchanged Tauri updater schemas", (t) => {
  const options = stagedRelease(t);
  const result = finalizeStandardRelease({ ...options, baseUrl: `${baseUrl}/`, notes: "Fixture" });
  const latest = JSON.parse(readFileSync(resolve(options.assetsDir, "latest.json"), "utf8"));
  const downloads = JSON.parse(readFileSync(resolve(options.assetsDir, "downloads.json"), "utf8"));
  assert.deepEqual(Object.keys(latest).sort(), ["notes", "platforms", "pub_date", "version"]);
  assert.equal(latest.version, version);
  assert.equal(latest.notes, "Fixture");
  assert.equal(new Date(latest.pub_date).toISOString(), latest.pub_date);
  assert.deepEqual(Object.keys(latest.platforms), Object.keys(updaterArtifacts));
  for (const [platform, name] of Object.entries(updaterArtifacts)) {
    assert.deepEqual(latest.platforms[platform], {
      signature: `fixture-${name}.sig`, url: `${baseUrl}/stable/${version}/${name}`,
    });
  }
  assert.doesNotMatch(JSON.stringify(latest), /\.dmg|downloads/);
  assert.deepEqual(downloads, createDownloadsManifest({ files, version, baseUrl }));
  assert.deepEqual(result, { files, platforms: latest.platforms, downloads: downloads.downloads });

  const checksums = readFileSync(resolve(options.assetsDir, "SHA256SUMS.txt"), "utf8");
  const expected = files.map((name) => {
    const hash = createHash("sha256").update(readFileSync(resolve(options.assetsDir, name))).digest("hex");
    return `${hash}  ${name}\n`;
  }).join("");
  assert.equal(checksums, expected);
  assert.doesNotMatch(checksums, /SHA256SUMS|latest\.json|downloads\.json/);
});

test("repeated finalize excludes stale manifests and the checksum file from immutable inventory", (t) => {
  const options = stagedRelease(t);
  finalizeStandardRelease(options);
  const checksums = readFileSync(resolve(options.assetsDir, "SHA256SUMS.txt"), "utf8");
  const downloads = readFileSync(resolve(options.assetsDir, "downloads.json"), "utf8");
  for (const name of metadata) writeFileSync(resolve(options.assetsDir, name), "stale metadata");
  const result = finalizeStandardRelease(options);
  assert.deepEqual(result.files, files);
  assert.equal(readFileSync(resolve(options.assetsDir, "SHA256SUMS.txt"), "utf8"), checksums);
  assert.equal(readFileSync(resolve(options.assetsDir, "downloads.json"), "utf8"), downloads);
});

test("a directory with a canonical installer name cannot satisfy artifact validation", (t) => {
  const options = stagedRelease(t);
  const installer = resolve(options.assetsDir, installers[1]);
  unlinkSync(installer);
  mkdirSync(installer);
  assert.throws(() => finalizeStandardRelease(options), /must be a regular file/);
  assertNoMetadata(options.assetsDir);
});

for (const [platform, name] of Object.entries(updaterArtifacts)) {
  for (const state of ["missing", "empty"]) {
    test(`requires a non-empty ${platform} updater signature (${state})`, (t) => {
      const options = stagedRelease(t);
      const signature = resolve(options.assetsDir, `${name}.sig`);
      if (state === "missing") unlinkSync(signature);
      else writeFileSync(signature, " \n");
      assert.throws(() => finalizeStandardRelease(options), /missing .*\.sig|signature .* is empty/);
      assertNoMetadata(options.assetsDir);
    });
  }
}

for (const platform of ["darwin-aarch64", "darwin-x86_64"]) {
  test(`macOS DMGs cannot replace the ${platform} updater archive`, (t) => {
    const options = stagedRelease(t);
    unlinkSync(resolve(options.assetsDir, updaterArtifacts[platform]));
    assert.throws(() => finalizeStandardRelease(options), /missing required updater artifact/);
    assertNoMetadata(options.assetsDir);
  });
}

test("rejects non-canonical Linux deb and rpm package assets", (t) => {
  const options = stagedRelease(t);
  for (const extension of ["deb", "rpm"]) {
    writeFileSync(resolve(options.assetsDir, `Unfour_${version}_linux_x64.${extension}`), "package");
  }
  assert.throws(() => finalizeStandardRelease(options), /Non-canonical Linux package assets must not be staged/);
  assertNoMetadata(options.assetsDir);
});

test("Standard workflow builds once and republishes the same staged files", () => {
  const releaseWorkflow = readFileSync(
    new URL("../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );
  const buildWorkflow = readFileSync(
    new URL("../.github/workflows/reusable-standard-build.yml", import.meta.url),
    "utf8",
  );
  assert.equal(buildWorkflow.match(/pnpm run tauri build/g)?.length, 1);
  assert.match(releaseWorkflow, /pattern: release-assets-\*/);
  assert.match(buildWorkflow, /linux_x64\.AppImage/);
  assert.match(buildWorkflow, /linux_x64\.AppImage\.sig/);
  assert.doesNotMatch(buildWorkflow, /bundle\/(?:deb|rpm)/);
  assert.doesNotMatch(buildWorkflow, /linux_x64\.(?:deb|rpm)/);
  assert.match(releaseWorkflow, /aws s3 cp release-assets[\s\S]*sha256sum -c/);
  assert.match(releaseWorkflow, /sha256sum -c[\s\S]*softprops\/action-gh-release/);
  assert.match(releaseWorkflow, /files: release-assets\/\*/);
  assert.match(releaseWorkflow, /Refusing to overwrite immutable R2 object/);
  assert.match(releaseWorkflow, /group: standard-stable-publish/);
  assert.match(releaseWorkflow, /cancel-in-progress: false/);
  assert.match(releaseWorkflow, /needs: \[identity, standard-build\]/);
  const versionedUpload = releaseWorkflow.split(/\r?\n/).find((line) =>
    line.includes('aws s3 cp release-assets "s3://${R2_BUCKET}/stable/${VERSION}/"'),
  );
  assert.match(versionedUpload, /--exclude latest\.json/);
  assert.match(versionedUpload, /--exclude downloads\.json/);
  for (const name of ["downloads", "latest"]) {
    const promotion = releaseWorkflow.split(/\r?\n/).filter((line) =>
      line.includes(`aws s3 cp release-assets/${name}.json`),
    );
    assert.equal(promotion.length, 1);
    assert.ok(promotion[0].includes(`"s3://\${R2_BUCKET}/stable/${name}.json"`));
    assert.match(promotion[0], /--content-type application\/json --cache-control no-cache/);
    assert.match(releaseWorkflow, new RegExp(`--${name}-url https://releases\\.unfour\\.dev/stable/${name}\\.json`));
  }

  const finalizeIndex = releaseWorkflow.indexOf("scripts/finalize-standard-release.mjs");
  const immutableComparisonIndex = releaseWorkflow.indexOf("cmp --silent");
  const versionedUploadIndex = releaseWorkflow.indexOf("aws s3 cp release-assets \"s3://${R2_BUCKET}/stable/${VERSION}/\"");
  const redownloadIndex = releaseWorkflow.indexOf("aws s3 cp \"s3://${R2_BUCKET}/stable/${VERSION}/\" r2-verify");
  const verifyIndex = releaseWorkflow.indexOf("sha256sum -c");
  const githubReleaseIndex = releaseWorkflow.indexOf("softprops/action-gh-release@v3");
  const updateOrderIndex = releaseWorkflow.indexOf("scripts/check-update-order.mjs");
  const downloadsUploadIndex = releaseWorkflow.indexOf("aws s3 cp release-assets/downloads.json");
  const latestUploadIndex = releaseWorkflow.indexOf("aws s3 cp release-assets/latest.json");
  for (const index of [finalizeIndex, immutableComparisonIndex, versionedUploadIndex, redownloadIndex, verifyIndex, githubReleaseIndex, updateOrderIndex, downloadsUploadIndex, latestUploadIndex]) {
    assert.notEqual(index, -1, "required publication stage must exist");
  }
  assert.ok(finalizeIndex < immutableComparisonIndex);
  assert.ok(immutableComparisonIndex < versionedUploadIndex);
  assert.ok(versionedUploadIndex < redownloadIndex);
  assert.ok(redownloadIndex < verifyIndex);
  assert.ok(verifyIndex < githubReleaseIndex);
  assert.ok(githubReleaseIndex < updateOrderIndex);
  assert.ok(updateOrderIndex < downloadsUploadIndex);
  assert.ok(downloadsUploadIndex < latestUploadIndex);
  const promotionStep = releaseWorkflow.slice(releaseWorkflow.lastIndexOf("      - name:"));
  assert.match(promotionStep, /set -euo pipefail[\s\S]*check-update-order[\s\S]*aws s3 cp/);
});
