export const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const LAST_UPDATE_CHECK_KEY = "unfour.updates.lastSuccessfulCheckAt";

export function wasUpdateCheckedRecently(now = Date.now()): boolean {
  try {
    const checkedAt = Number(window.localStorage.getItem(LAST_UPDATE_CHECK_KEY));
    return Number.isFinite(checkedAt)
      && checkedAt > 0
      && now >= checkedAt
      && now - checkedAt < UPDATE_CHECK_INTERVAL_MS;
  } catch {
    return false;
  }
}

export function recordSuccessfulUpdateCheck(checkedAt = Date.now()): void {
  try {
    window.localStorage.setItem(LAST_UPDATE_CHECK_KEY, String(checkedAt));
  } catch {
    // Storage availability must not affect updater behavior.
  }
}
