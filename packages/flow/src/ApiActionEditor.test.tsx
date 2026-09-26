// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { I18nProvider } from "@unfour/ui";
import type { FlowAction } from "@unfour/command-client";
import { ApiActionEditor } from "./ApiActionEditor";
import { sensitiveKey } from "./model";
afterEach(cleanup);
const pair = (value: string, occurrence: number, enabled = true) => ({ key: "tag", value, enabled, occurrence });
function Editor({ changed, legacy = false }: { changed: (action: FlowAction) => void; legacy?: boolean }) {
  const [action, setAction] = useState<FlowAction>({ capability: "api", resourceId: "api", arguments: legacy ? { queryPatch: [{ key: "tag", value: "A", enabled: true }, { key: "tag", value: "B", enabled: true }] } : {} });
  return <I18nProvider initialLocale="en"><ApiActionEditor action={action} resources={{ api: [{ id: "api", name: "API", queryJson: JSON.stringify([{ key: "tag", value: "a", enabled: true }, { key: "tag", value: "b", enabled: true }]) }], ssh: [], database: [], connections: [] }} variables={[]} onValidity={() => {}} onChange={(next) => { setAction(next); changed(next); }} /></I18nProvider>;
}
it.each([0, 1])("targets query occurrence %i and restores only that occurrence", (target) => {
  const changed = vi.fn();
  render(<Editor changed={changed} />);
  const query = within(screen.getByRole("group", { name: "Query parameters" }));
  fireEvent.click(query.getAllByRole("button", { name: "Override", exact: true })[target]);
  fireEvent.change(screen.getByLabelText("Query parameters 1", { exact: true }), { target: { value: "edited" } });
  expect(changed.mock.lastCall?.[0].arguments.queryPatch).toEqual([pair("edited", target)]);
  fireEvent.click(query.getByRole("button", { name: "Override", exact: true }));
  expect(changed.mock.lastCall?.[0].arguments.queryPatch).toEqual([pair("edited", target), pair(target === 0 ? "b" : "a", 1 - target)]);
  fireEvent.click(screen.getByLabelText("Query parameters 1 · Enabled"));
  expect(changed.mock.lastCall?.[0].arguments.queryPatch[0]).toEqual(pair("edited", target, false));
  fireEvent.click(screen.getByLabelText("Query parameters 2 · Enabled"));
  expect(changed.mock.lastCall?.[0].arguments.queryPatch[1].enabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Query parameters 1 · Remove" }));
  expect(changed.mock.lastCall?.[0].arguments.queryPatch).toEqual([pair(target === 0 ? "b" : "a", 1 - target, false)]);
  expect(query.getAllByRole("button", { name: "Override", exact: true })).toHaveLength(1);
});
it("freezes legacy sequential query targets before restoring the first", () => {
  const changed = vi.fn();
  render(<Editor changed={changed} legacy />);
  fireEvent.click(screen.getByRole("button", { name: "Query parameters 1 · Remove" }));
  expect(changed.mock.lastCall?.[0].arguments.queryPatch).toEqual([pair("B", 1)]);
});
it.each(["DEPLOY_TOKEN", "DB_PASSWORD", "apiKey", "privateKey", "passphrase", "credential", "license_key"])("recognizes sensitive name %s", (name) => expect(sensitiveKey(name)).toBe(true));
it.each(["DEPLOY", "VERSION", "buildId"])("keeps ordinary name %s visible", (name) => expect(sensitiveKey(name)).toBe(false));

