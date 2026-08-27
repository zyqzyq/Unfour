import assert from "node:assert/strict";
import test from "node:test";

import { validateStandardStableRelease } from "./release-contract.mjs";

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
