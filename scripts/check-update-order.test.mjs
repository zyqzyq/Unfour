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

const downloadsUrl = "https://releases.unfour.dev/stable/downloads.json";

function checkBoth(candidateVersion, updaterVersion, downloadResponse) {
  return checkUpdateOrder({
    candidateVersion,
    downloadsUrl,
    fetchImpl: async (url) => {
      if (url === DEFAULT_LATEST_URL) return latestResponse(updaterVersion);
      assert.equal(url, downloadsUrl);
      return downloadResponse;
    },
  });
}

test("allows an initial downloads.json migration without requiring historical backfill", async () => {
  const decision = await checkBoth("0.10.0", "0.9.0", { status: 404, ok: false });
  assert.equal(decision.allowed, true);
});

test("checks both stable manifests before allowing newer or equal publication", async () => {
  for (const candidateVersion of ["0.10.0", "0.10.1"]) {
    const urls = [];
    const decision = await checkUpdateOrder({
      candidateVersion,
      downloadsUrl,
      fetchImpl: async (url) => {
        urls.push(url);
        return latestResponse("0.10.0");
      },
    });
    assert.equal(decision.allowed, true);
    assert.deepEqual(urls, [DEFAULT_LATEST_URL, downloadsUrl]);
  }
});

test("blocks a downloads downgrade after downloads promotion succeeded but updater promotion failed", async () => {
  await assert.rejects(
    () => checkBoth("0.10.0", "0.9.0", latestResponse("0.10.1")),
    /downloads\.json: candidate 0\.10\.0 is older than current 0\.10\.1/,
  );
  const retry = await checkBoth("0.10.1", "0.9.0", latestResponse("0.10.1"));
  assert.equal(retry.allowed, true);
});

test("an older updater candidate cannot promote either pointer even if downloads is older", async () => {
  await assert.rejects(
    () => checkBoth("0.10.0", "0.10.1", latestResponse("0.9.0")),
    /latest\.json: candidate 0\.10\.0 is older than current 0\.10\.1/,
  );
});

test("a missing updater pointer does not bypass the downloads version gate", async () => {
  await assert.rejects(
    () => checkUpdateOrder({
      candidateVersion: "0.10.0",
      downloadsUrl,
      fetchImpl: async (url) => url === DEFAULT_LATEST_URL
        ? { status: 404, ok: false }
        : latestResponse("0.10.1"),
    }),
    /downloads\.json: candidate 0\.10\.0 is older/,
  );
});

test("fails closed for unreadable or malformed downloads metadata", async () => {
  for (const response of [
    { status: 403, ok: false },
    { status: 503, ok: false },
    { status: 200, ok: true, json: async () => { throw new Error("invalid JSON"); } },
    { status: 200, ok: true, json: async () => null },
    { status: 200, ok: true, json: async () => [] },
    latestResponse(undefined),
    latestResponse("0.10.0-rc.1"),
    latestResponse("0.10.0\n"),
  ]) {
    await assert.rejects(
      () => checkBoth("0.10.0", "0.9.0", response),
      /downloads\.json|Stable release version must be X\.Y\.Z/,
    );
  }
  await assert.rejects(
    () => checkUpdateOrder({
      candidateVersion: "0.10.0",
      downloadsUrl,
      fetchImpl: async (url) => {
        if (url === DEFAULT_LATEST_URL) return latestResponse("0.9.0");
        throw new Error("network unavailable");
      },
    }),
    /downloads\.json: network unavailable/,
  );
});
