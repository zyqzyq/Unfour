import * as React from "react";
import { ThemeContext, type ThemeContextValue, type ThemeMode } from "./theme-context";
import {
  applyTheme,
  readStoredThemeMode,
  resolveTheme,
  writeStoredThemeMode,
} from "./theme-internal";

const DEFAULT_THEME_MODE: ThemeMode = "dark";
const DEFAULT_STORAGE_KEY = "unfour.theme";
const SYSTEM_MEDIA_QUERY = "(prefers-color-scheme: dark)";

function subscribeSystemTheme(onChange: () => void) {
  const media = globalThis.matchMedia?.(SYSTEM_MEDIA_QUERY);
  media?.addEventListener("change", onChange);
  return () => media?.removeEventListener("change", onChange);
}

export function ThemeProvider({
  children,
  defaultThemeMode = DEFAULT_THEME_MODE,
  storageKey = DEFAULT_STORAGE_KEY,
}: {
  children: React.ReactNode;
  defaultThemeMode?: ThemeMode;
  storageKey?: string;
}) {
  const [themeMode, setThemeModeState] = React.useState<ThemeMode>(
    () => readStoredThemeMode(storageKey) ?? defaultThemeMode,
  );
  const theme = React.useSyncExternalStore(
    subscribeSystemTheme,
    () => resolveTheme(themeMode),
  );

  // Resolve and apply the theme whenever the user preference changes.
  React.useLayoutEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const setThemeMode = React.useCallback(
    (nextMode: ThemeMode) => {
      setThemeModeState(nextMode);
      writeStoredThemeMode(storageKey, nextMode);
    },
    [storageKey],
  );

  const value = React.useMemo<ThemeContextValue>(
    () => ({ setThemeMode, theme, themeMode }),
    [setThemeMode, theme, themeMode],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
