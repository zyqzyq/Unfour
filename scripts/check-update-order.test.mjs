import assert from "node:assert/strict";
import test from "node:test";

import {
  checkUpdateOrder,
  compareSemVer,
  DEFAULT_LATEST_URL,
} from "./check-update-order.mjs";

function latestResponse(version) {
  return {
    status: 200,
    ok: true,
    json: async () => ({ version }),
  };
}

function checkAgainst(candidateVersion, currentVersion) {
  return checkUpdateOrder({
    candidateVersion,
    fetchImpl: async (url, options) => {
      assert.equal(url, DEFAULT_LATEST_URL);
      assert.deepEqual(options, { headers: { accept: "application/json" } });
      return latestResponse(currentVersion);
    },
  });
}

test("compares SemVer numeric fields instead of version strings", () => {
  assert.equal(compareSemVer("0.10.0", "0.9.0"), 1);
  assert.equal(compareSemVer("0.9.0", "0.10.0"), -1);
});

test("allows 0.9.0 to advance to 0.10.0", async () => {
  const decision = await checkAgainst("0.10.0", "0.9.0");
  assert.equal(decision.allowed, true);
  assert.equal(decision.relation, "newer");
});

test("allows 0.10.0 to advance to 0.10.1", async () => {
  const decision = await checkAgainst("0.10.1", "0.10.0");
  assert.equal(decision.allowed, true);
  assert.equal(decision.relation, "newer");
});

test("rejects a candidate that would downgrade 0.10.0 to 0.9.0", async () => {
  await assert.rejects(
    () => checkAgainst("0.9.0", "0.10.0"),
    /candidate 0\.9\.0 is older than current 0\.10\.0/,
  );
});

test("allows an equal version for an idempotent rerun", async () => {
  const decision = await checkAgainst("0.10.0", "0.10.0");
  assert.equal(decision.allowed, true);
  assert.equal(decision.relation, "equal");
});

test("allows the first publication when latest.json is missing", async () => {
  const decision = await checkUpdateOrder({
    candidateVersion: "0.9.0",
    fetchImpl: async (url) => {
      assert.equal(url, DEFAULT_LATEST_URL);
      return { status: 404, ok: false };
    },
  });
  assert.deepEqual(decision, {
    allowed: true,
    candidateVersion: "0.9.0",
    currentVersion: null,
    relation: "missing",
  });
});

test("fails closed when the current manifest cannot be read", async () => {
  await assert.rejects(
    () => checkUpdateOrder({
      candidateVersion: "0.10.0",
      fetchImpl: async () => ({ status: 503, ok: false }),
    }),
    /HTTP 503/,
  );
});

test("fails closed when the current manifest has an invalid version", async () => {
  await assert.rejects(
    () => checkAgainst("0.10.0", "0.10.0-rc.1"),
    /Stable release version must be X\.Y\.Z/,
  );
});
