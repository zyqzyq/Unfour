import { createContext, useContext } from "react";

export type Theme = "light" | "dark";
export type ThemeMode = "light" | "dark" | "system";
export type ThemeContextValue = {
  setThemeMode: (mode: ThemeMode) => void;
  theme: Theme;
  themeMode: ThemeMode;
};

export const ThemeContext = createContext<ThemeContextValue>({
  setThemeMode: () => undefined,
  theme: "dark",
  themeMode: "dark",
});

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}
