import type { RequestParamsTab } from "./types";

export const requestConfigTabs: Array<{ id: RequestParamsTab; labelKey: string }> = [
  { id: "query", labelKey: "api.request.tabs.params" },
  { id: "auth", labelKey: "api.request.tabs.auth" },
  { id: "headers", labelKey: "api.request.tabs.headers" },
  { id: "body", labelKey: "api.request.tabs.body" },
  { id: "scripts", labelKey: "api.request.tabs.scripts" },
  { id: "settings", labelKey: "api.request.tabs.settings" },
];

export function methodBadgeLabel(method: string) {
  const normalized = method.trim().toUpperCase();
  return normalized === "DELETE" ? "DEL" : normalized;
}

export function methodToneClass(method: string) {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "text-[color:var(--u-color-info-text)]";
    case "POST":
      return "text-[color:var(--u-color-success)]";
    case "PUT":
      return "text-[color:var(--u-color-warning-text)]";
    case "PATCH":
      return "text-[color:var(--u-color-primary)]";
    case "DELETE":
      return "text-[color:var(--u-color-danger)]";
    case "HEAD":
      return "text-[color:var(--u-color-secondary-text)]";
    case "OPTIONS":
      return "text-[color:var(--u-color-neutral-text)]";
    default:
      return "text-[color:var(--u-color-text-muted)]";
  }
}

export function methodBadgeToneClass(method: string) {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "bg-[var(--u-color-info-soft)] text-[color:var(--u-color-info-text)]";
    case "POST":
      return "bg-[var(--u-color-success-soft)] text-[color:var(--u-color-success)]";
    case "PUT":
      return "bg-[var(--u-color-warning-soft)] text-[color:var(--u-color-warning-text)]";
    case "PATCH":
      return "bg-[var(--u-color-primary-soft)] text-[color:var(--u-color-primary)]";
    case "DELETE":
      return "bg-[var(--u-color-danger-soft)] text-[color:var(--u-color-danger-text)]";
    case "HEAD":
      return "bg-[var(--u-color-secondary-soft)] text-[color:var(--u-color-secondary-text)]";
    case "OPTIONS":
      return "bg-[var(--u-color-neutral-soft)] text-[color:var(--u-color-neutral-text)]";
    default:
      return "bg-[var(--u-color-surface-muted)] text-[color:var(--u-color-text-muted)]";
  }
}
