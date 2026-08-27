import { describe, expect, it } from "vitest";
import {
  getActiveCloudSyncEntitlement,
  hasActiveEntitlement,
  CLOUD_SYNC_ENTITLEMENT,
  TEAM_WORKSPACE_ENTITLEMENT,
} from "./accountEntitlement";
import type { AccountProfile, EntitlementSummary } from "./accountTypes";

const now = new Date("2026-08-23T00:00:00.000Z");

function entitlement(
  overrides: Partial<EntitlementSummary> = {},
): EntitlementSummary {
  return {
    code: CLOUD_SYNC_ENTITLEMENT,
    status: "active",
    validUntil: null,
    ...overrides,
  };
}

describe("capability entitlements", () => {
  it("reserves stable capability codes without enabling Team Workspace", () => {
    expect(CLOUD_SYNC_ENTITLEMENT).toBe("cloud_sync");
    expect(TEAM_WORKSPACE_ENTITLEMENT).toBe("team_workspace");
  });
  it("accepts active cloud_sync with no expiry", () => {
    expect(getActiveCloudSyncEntitlement([entitlement()], now)).not.toBeNull();
  });

  it("accepts active cloud_sync with a future expiry", () => {
    expect(getActiveCloudSyncEntitlement([
      entitlement({ validUntil: "2026-09-23T00:00:00.000Z" }),
    ], now)).not.toBeNull();
  });

  it("rejects active cloud_sync after its expiry", () => {
    expect(getActiveCloudSyncEntitlement([
      entitlement({ validUntil: "2026-08-22T23:59:59.000Z" }),
    ], now)).toBeNull();
  });

  it.each(["expired", "revoked"] as const)("rejects %s cloud_sync", (status) => {
    expect(getActiveCloudSyncEntitlement([entitlement({ status })], now)).toBeNull();
  });

  it("keeps suspended in the signed-in profile without granting Pro", () => {
    const profile: AccountProfile = {
      id: "account-a",
      email: "account-a@example.test",
      username: "account-a",
      displayName: "Account A",
      avatarUrl: null,
      entitlements: [entitlement({ status: "suspended" })],
      devices: [],
    };

    expect(profile.entitlements[0]?.status).toBe("suspended");
    expect(getActiveCloudSyncEntitlement(profile.entitlements, now)).toBeNull();
    expect(hasActiveEntitlement(profile.entitlements, CLOUD_SYNC_ENTITLEMENT, now)).toBe(false);
  });

  it("rejects unrelated active entitlements, including the legacy pro code", () => {
    expect(getActiveCloudSyncEntitlement([
      entitlement({ code: "pro" }),
      entitlement({ code: "other" }),
    ], now)).toBeNull();
    expect(hasActiveEntitlement([entitlement({ code: "pro" })], CLOUD_SYNC_ENTITLEMENT, now))
      .toBe(false);
  });

  it("moves from Free to Pro when refresh supplies cloud_sync", () => {
    const before: EntitlementSummary[] = [];
    const after = [entitlement()];
    expect(getActiveCloudSyncEntitlement(before, now)).toBeNull();
    expect(getActiveCloudSyncEntitlement(after, now)).not.toBeNull();
  });

  it("moves from Pro to Free when refreshed cloud_sync is expired", () => {
    const before = [entitlement()];
    const after = [entitlement({ status: "expired" })];
    expect(getActiveCloudSyncEntitlement(before, now)).not.toBeNull();
    expect(getActiveCloudSyncEntitlement(after, now)).toBeNull();
  });
});
