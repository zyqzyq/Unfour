import {
  applyTheme,
  readStoredThemeMode,
  resolveTheme,
} from "./theme-internal";
import type { Theme, ThemeMode } from "./theme-context";

const DEFAULT_THEME_MODE: ThemeMode = "dark";
const DEFAULT_STORAGE_KEY = "unfour.theme";

export function initializeTheme({
  defaultThemeMode = DEFAULT_THEME_MODE,
  storageKey = DEFAULT_STORAGE_KEY,
}: {
  defaultThemeMode?: ThemeMode;
  storageKey?: string;
} = {}): Theme {
  const mode = readStoredThemeMode(storageKey) ?? defaultThemeMode;
  const theme = resolveTheme(mode);
  applyTheme(theme);
  return theme;
}
