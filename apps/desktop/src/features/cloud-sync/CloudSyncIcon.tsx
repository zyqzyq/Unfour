import type { CloudSyncViewState } from "./syncTypes";

export function CloudSyncIcon({ state, size = 14 }: { state: CloudSyncViewState; size?: number }) {
  const color = ["attention", "auth_required"].includes(state)
    ? "var(--u-color-danger)"
    : state === "capability_required"
      ? "var(--u-color-warning)"
    : state === "offline"
      ? "var(--u-color-warning)"
      : state === "synced"
        ? "var(--u-color-success)"
        : "var(--u-color-text-soft)";
  return (
    <svg
      aria-hidden="true"
      className={state === "syncing" ? "animate-pulse" : undefined}
      fill="none"
      height={size}
      style={{ color }}
      viewBox="0 0 24 24"
      width={size}
    >
      <path d="M7 18h10a4 4 0 0 0 .7-7.94A6 6 0 0 0 6.24 8.3 4.5 4.5 0 0 0 7 18Z" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
      {state === "synced" && <path d="m9.5 13 1.7 1.7 3.5-3.7" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />}
      {state === "paused" && <path d="M10 11v4m4-4v4" stroke="currentColor" strokeLinecap="round" strokeWidth="1.8" />}
      {["attention", "auth_required", "capability_required"].includes(state) && <path d="M12 10v3m0 2.5h.01" stroke="currentColor" strokeLinecap="round" strokeWidth="1.8" />}
      {state === "offline" && <path d="m9.5 11 5 5m0-5-5 5" stroke="currentColor" strokeLinecap="round" strokeWidth="1.8" />}
    </svg>
  );
}
