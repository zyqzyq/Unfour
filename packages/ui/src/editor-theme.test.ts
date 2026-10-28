import { readFileSync } from "node:fs";
import { expect, it, vi } from "vitest";
import { configureEditorThemes } from "./editor-theme";

it.each(["light", "dark"] as const)("keeps %s Monaco canvases and readable metadata aligned with CSS", (mode) => {
  const defineTheme = vi.fn();
  const setTheme = vi.fn();
  // The helper uses only this presentation API, not a running editor.
  configureEditorThemes({ editor: { defineTheme, setTheme } }, mode);
  const css = readFileSync(new URL(`./styles/${mode === "light" ? "tokens" : "theme"}.css`, import.meta.url), "utf8");
  const token = (name: string) => css.match(new RegExp(`--u-color-${name}:\\s*(#[a-fA-F0-9]+)`))?.[1];
  const definition = defineTheme.mock.calls.find(([name]) => name === `unfour-${mode}`)?.[1];
  expect(definition.colors["editor.background"]).toBe(token("canvas"));
  expect(definition.colors["editor.foreground"]).toBe(token("text"));
  expect(definition.colors["editorLineNumber.foreground"]).toBe(token("text-muted"));
  expect(`#${definition.rules.find((rule: { token: string }) => rule.token === "comment").foreground}`).toBe(token("text-muted"));
  expect(setTheme).toHaveBeenCalledWith(`unfour-${mode}`);
});
