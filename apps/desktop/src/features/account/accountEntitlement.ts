import type { EntitlementSummary } from "./accountTypes";

export const CLOUD_SYNC_ENTITLEMENT = "cloud_sync";
export const TEAM_WORKSPACE_ENTITLEMENT = "team_workspace";

export function getActiveCloudSyncEntitlement(
  entitlements: readonly EntitlementSummary[],
  now = new Date(),
): EntitlementSummary | null {
  return entitlements.find((entitlement) =>
    isActiveEntitlement(entitlement, CLOUD_SYNC_ENTITLEMENT, now)
  ) ?? null;
}

export function hasActiveEntitlement(
  entitlements: readonly EntitlementSummary[],
  code: string,
  now = new Date(),
): boolean {
  return entitlements.some((entitlement) => isActiveEntitlement(entitlement, code, now));
}

function isActiveEntitlement(
  entitlement: EntitlementSummary,
  code: string,
  now: Date,
): boolean {
  if (entitlement.code !== code || entitlement.status !== "active") return false;
  if (!entitlement.validUntil) return true;
  const validUntil = new Date(entitlement.validUntil);
  return !Number.isNaN(validUntil.getTime()) && validUntil > now;
}

export function formatAccountDate(
  value: string | null | undefined,
  locale: string,
): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(date);
}
