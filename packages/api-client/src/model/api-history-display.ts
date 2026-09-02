import type { ApiHistoryItem } from "@unfour/command-client";

type ApiHistoryDisplayItem = Pick<ApiHistoryItem, "method" | "name" | "url">;

export function apiHistoryRequestName(
  item: Pick<ApiHistoryDisplayItem, "name">,
): string | null {
  const name = item.name?.trim();
  return name || null;
}

export function apiHistoryPath(rawUrl: string): string {
  const value = rawUrl.trim();
  if (!value) {
    return "/";
  }
  if (/^(?:localhost|(?:\d{1,3}\.){3}\d{1,3}|\[[^\]]+\])(?::\d+)?\//i.test(value)) {
    return fallbackPath(value);
  }

  try {
    const parsed = new URL(value);
    return parsed.pathname || "/";
  } catch {
    return fallbackPath(value);
  }
}

export function apiHistoryPrimaryLabel(item: ApiHistoryDisplayItem): string {
  return apiHistoryRequestName(item) ?? apiHistoryPath(item.url);
}

export function apiHistoryTooltip(item: ApiHistoryDisplayItem): string {
  const method = item.method.trim().toUpperCase();
  return method ? `${method}\n${item.url}` : item.url;
}

function fallbackPath(rawUrl: string): string {
  const withoutFragment = rawUrl.split("#", 1)[0];
  const queryIndex = withoutFragment.indexOf("?");
  const value = queryIndex >= 0
    ? withoutFragment.slice(0, queryIndex)
    : withoutFragment;

  if (!value) {
    return "/";
  }
  if (value.startsWith("/")) {
    return value;
  }

  const schemeEnd = value.indexOf("://");
  if (schemeEnd >= 0) {
    return pathAfterAuthority(value, schemeEnd + 3);
  }
  if (value.startsWith("//")) {
    return pathAfterAuthority(value, 2);
  }

  if (/^(?:localhost|(?:\d{1,3}\.){3}\d{1,3}|\[[^\]]+\])(?::\d+)?\//i.test(value)) {
    return value.slice(value.indexOf("/"));
  }

  return value;
}

function pathAfterAuthority(value: string, authorityStart: number): string {
  const pathStart = value.indexOf("/", authorityStart);
  return pathStart >= 0 ? value.slice(pathStart) : "/";
}
