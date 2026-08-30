import type { Theme, ThemeMode } from "./theme-context";

export function applyTheme(theme: Theme) {
  globalThis.document?.documentElement.setAttribute("data-theme", theme);
}

/**
 * Reads the webview's preferred color scheme via the standard
 * `prefers-color-scheme` media query. This runs inside the webview and is
 * fast; it deliberately avoids any native OS theme API (which can stall the
 * UI on Windows).
 */
export function getSystemTheme(): Theme {
  if (typeof globalThis.matchMedia !== "function") {
    return "light";
  }
  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function resolveTheme(mode: ThemeMode): Theme {
  return mode === "system" ? getSystemTheme() : mode;
}

export function readStoredThemeMode(storageKey: string): ThemeMode | null {
  try {
    const stored = globalThis.localStorage?.getItem(storageKey);
    return stored === "light" || stored === "dark" || stored === "system"
      ? stored
      : null;
  } catch {
    return null;
  }
}

export function writeStoredThemeMode(storageKey: string, mode: ThemeMode) {
  try {
    globalThis.localStorage?.setItem(storageKey, mode);
  } catch {
    // Ignore storage failures; theme still applies for the current session.
  }
}
