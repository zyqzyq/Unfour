import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { resolveBuildProfile } from "./release-channel.mjs";
import {
  validateStandardStableRelease,
  validateTauriUpdaterConfigFiles,
  validateTauriUpdaterConfigs,
} from "./release-contract.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function validTauriConfig() {
  return {
    bundle: {},
    plugins: {
      updater: {
        active: true,
        pubkey: "tracked-public-key",
        windows: { installMode: "passive" },
      },
    },
  };
}

function validReleaseConfig() {
  return {
    $schema: "https://schema.tauri.app/config/2",
    bundle: { createUpdaterArtifacts: true },
  };
}

test("Standard Stable accepts only the exact X.Y.Z workspace tag", () => {
  assert.deepEqual(validateStandardStableRelease("1.2.3", "v1.2.3"), {
    version: "1.2.3",
    tag: "v1.2.3",
    channel: "stable",
    distribution: "standard",
    updaterEndpoint: "https://release.unfour.dev/stable/latest.json",
    prerelease: false,
  });
  assert.throws(
    () => validateStandardStableRelease("1.2.3", "v1.2.4"),
    /must exactly match v1\.2\.3/,
  );
});

test("pre-release and four-part project versions cannot enter Standard Stable", () => {
  for (const version of ["1.2.3-test.1", "1.2.3-dev", "1.2.3-rc.1", "1.2.3.0"]) {
    assert.throws(
      () => validateStandardStableRelease(version, `v${version}`),
      /must use X\.Y\.Z/,
    );
  }
});

test("tracked Tauri configs separate updater runtime from artifact generation", () => {
  assert.doesNotThrow(() => validateTauriUpdaterConfigFiles(repoRoot));
});

test("base Tauri config must always provide the updater runtime contract", () => {
  const missingUpdater = validTauriConfig();
  delete missingUpdater.plugins.updater;
  assert.throws(
    () => validateTauriUpdaterConfigs(
      missingUpdater,
      validReleaseConfig(),
      "tracked-public-key\n",
    ),
    /must define plugins\.updater/,
  );

  const mismatchedKey = validTauriConfig();
  mismatchedKey.plugins.updater.pubkey = "different-public-key";
  assert.throws(
    () => validateTauriUpdaterConfigs(
      mismatchedKey,
      validReleaseConfig(),
      "tracked-public-key\n",
    ),
    /must exactly match updater_secret\.key\.pub/,
  );
});

test("base config cannot generate release artifacts", () => {
  const tauri = validTauriConfig();
  tauri.bundle.createUpdaterArtifacts = true;
  assert.throws(
    () => validateTauriUpdaterConfigs(
      tauri,
      validReleaseConfig(),
      "tracked-public-key\n",
    ),
    /must not create updater artifacts/,
  );
});

test("release config only enables updater artifact generation", () => {
  const missingArtifactFlag = validReleaseConfig();
  delete missingArtifactFlag.bundle.createUpdaterArtifacts;
  assert.throws(
    () => validateTauriUpdaterConfigs(
      validTauriConfig(),
      missingArtifactFlag,
      "tracked-public-key\n",
    ),
    /must set bundle\.createUpdaterArtifacts=true/,
  );

  const duplicatedRuntime = validReleaseConfig();
  duplicatedRuntime.plugins = {
    updater: { pubkey: "tracked-public-key" },
  };
  assert.throws(
    () => validateTauriUpdaterConfigs(
      validTauriConfig(),
      duplicatedRuntime,
      "tracked-public-key\n",
    ),
    /must not duplicate plugins\.updater runtime config/,
  );
});

test("base updater config does not grant updater authority to Store builds", () => {
  assert.doesNotThrow(() => validateTauriUpdaterConfigFiles(repoRoot));
  const store = resolveBuildProfile("0.8.0", "stable", "microsoft-store");
  assert.equal(store.updaterEnabled, false);
  assert.equal(store.updaterEndpoint, null);
});

test("only the shared Standard build core loads the artifact override", () => {
  const releaseWorkflow = readFileSync(
    resolve(repoRoot, ".github/workflows/release.yml"),
    "utf8",
  );
  const candidateWorkflow = readFileSync(
    resolve(repoRoot, ".github/workflows/release-candidate.yml"),
    "utf8",
  );
  const buildWorkflow = readFileSync(
    resolve(repoRoot, ".github/workflows/reusable-standard-build.yml"),
    "utf8",
  );
  const tauriRunner = readFileSync(
    resolve(repoRoot, "scripts/run-tauri.mjs"),
    "utf8",
  );
  const msixBuild = readFileSync(
    resolve(repoRoot, "scripts/msix/build-msix.ps1"),
    "utf8",
  );

  assert.match(
    buildWorkflow,
    /tauri build --config src-tauri\/tauri\.release\.conf\.json/,
  );
  assert.match(
    releaseWorkflow,
    /uses: \.\/\.github\/workflows\/reusable-standard-build\.yml/,
  );
  assert.match(
    candidateWorkflow,
    /uses: \.\/\.github\/workflows\/reusable-standard-build\.yml/,
  );
  assert.doesNotMatch(releaseWorkflow, /tauri build/);
  assert.doesNotMatch(candidateWorkflow, /tauri build/);
  assert.doesNotMatch(tauriRunner, /tauri\.release\.conf\.json/);
  assert.doesNotMatch(msixBuild, /tauri\.release\.conf\.json/);
});
