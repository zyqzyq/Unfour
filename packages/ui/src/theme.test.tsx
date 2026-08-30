// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ThemeProvider } from "./theme";
import { useTheme } from "./theme-context";

afterEach(() => { cleanup(); vi.unstubAllGlobals(); localStorage.clear(); });

function Probe() {
  const { theme, setThemeMode } = useTheme();
  return <>
    <output>{theme}</output>
    <button onClick={() => setThemeMode("dark")}>dark</button>
    <button onClick={() => setThemeMode("system")}>system</button>
  </>;
}

it("tracks system theme without cascading state and disposes its one subscription", () => {
  let matches = false;
  const listeners = new Set<() => void>();
  const add = vi.fn((_event: string, fn: () => void) => listeners.add(fn));
  const remove = vi.fn((_event: string, fn: () => void) => listeners.delete(fn));
  vi.stubGlobal("matchMedia", () => ({ get matches() { return matches; }, addEventListener: add, removeEventListener: remove }));
  const { rerender, unmount } = render(<ThemeProvider defaultThemeMode="system"><Probe /></ThemeProvider>);
  expect(screen.getByRole("status")).toHaveTextContent("light");
  act(() => { matches = true; listeners.forEach((listener) => listener()); });
  expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  fireEvent.click(screen.getByRole("button", { name: "dark" }));
  act(() => { matches = false; listeners.forEach((listener) => listener()); });
  expect(screen.getByRole("status")).toHaveTextContent("dark");
  fireEvent.click(screen.getByRole("button", { name: "system" }));
  expect(screen.getByRole("status")).toHaveTextContent("light");
  expect(localStorage.getItem("unfour.theme")).toBe("system");
  rerender(<ThemeProvider defaultThemeMode="system"><Probe /></ThemeProvider>);
  expect(add).toHaveBeenCalledTimes(1);
  unmount();
  expect(remove).toHaveBeenCalledTimes(1);
  expect(listeners.size).toBe(0);
});
