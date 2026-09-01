import assert from "node:assert/strict";
import test from "node:test";

import {
  resolveBuildProfile,
  resolveMsixPackageProfile,
} from "./release-channel.mjs";

test("Standard Stable and Test both enable their updater endpoints", () => {
  const stable = resolveBuildProfile("0.8.0", "stable", "standard");
  const testProfile = resolveBuildProfile("0.8.0", "test", "standard");

  assert.equal(stable.updaterEnabled, true);
  assert.equal(
    stable.updaterEndpoint,
    "https://releases.unfour.dev/stable/latest.json",
  );
  assert.equal(testProfile.updaterEnabled, true);
  assert.equal(
    testProfile.updaterEndpoint,
    "https://releases.unfour.dev/test/latest.json",
  );
});

test("only Stable builds receive the production telemetry endpoint", () => {
  const stable = resolveBuildProfile("0.8.0", "stable", "standard");
  const testProfile = resolveBuildProfile("0.8.0", "test", "standard");

  assert.equal(
    stable.telemetryEndpoint,
    "https://telemetry.unfour.dev/v1/active",
  );
  assert.equal(testProfile.telemetryEndpoint, null);
});

test("standard and Store are independent distribution authorities", () => {
  const standard = resolveBuildProfile("0.8.0", "stable", "standard");
  const store = resolveBuildProfile("0.8.0", "stable", "microsoft-store");

  assert.equal(standard.distribution, "standard");
  assert.equal(standard.updaterEnabled, true);
  assert.equal(
    standard.updaterEndpoint,
    "https://releases.unfour.dev/stable/latest.json",
  );
  assert.equal(store.distribution, "microsoft-store");
  assert.equal(store.updaterEnabled, false);
  assert.equal(store.updaterEndpoint, null);
  assert.equal(store.accountApiUrl, standard.accountApiUrl);
  assert.equal(store.telemetryEndpoint, standard.telemetryEndpoint);
  assert.equal(store.defaultStorageProfile, standard.defaultStorageProfile);
});

test("Store MSIX maps X.Y.Z to X.Y.Z.0 and cannot enable the NSIS updater", () => {
  const store = resolveMsixPackageProfile(
    resolveBuildProfile("1.2.3", "stable", "microsoft-store"),
    "Store",
  );
  assert.equal(store.msixVersion, "1.2.3.0");
  assert.equal(store.updaterEnabled, false);
  assert.equal(store.updaterEndpoint, null);
  assert.throws(
    () => resolveMsixPackageProfile(
      resolveBuildProfile("1.2.3", "stable", "standard"),
      "Store",
    ),
    /requires distribution=microsoft-store/,
  );
  assert.throws(
    () => resolveMsixPackageProfile(
      resolveBuildProfile("1.2.3", "test", "microsoft-store"),
      "Store",
    ),
    /requires a Stable X.Y.Z build/,
  );
});

test("release versions remain one three-part source across Cargo, package, and Tauri", () => {
  for (const version of ["1.2.3-test.1", "1.2.3-dev", "1.2.3.0", "01.2.3"]) {
    assert.throws(() => resolveBuildProfile(version), /must use X\.Y\.Z/);
  }
});
