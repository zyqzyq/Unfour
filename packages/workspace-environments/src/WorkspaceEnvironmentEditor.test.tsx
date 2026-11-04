// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import type { WorkspaceEnvironment, WorkspaceVariable } from "@unfour/command-client";
import { WorkspaceEnvironmentEditor, type EnvironmentEditorMode } from "./WorkspaceEnvironmentEditor";

afterEach(cleanup);
const variable: WorkspaceVariable = { id: "v", workspaceId: "w", key: "HOST", value: "old", isSecret: false, isEnabled: true, description: null, sortOrder: 0, createdAt: "t", updatedAt: "t", deletedAt: null, revision: 1 };
const environment: WorkspaceEnvironment = { id: "e", workspaceId: "w", name: "Dev", sortOrder: 0, isActive: false, variables: [{ ...variable, environmentId: "e" }], createdAt: "t", updatedAt: "t", deletedAt: null, revision: 1 };
function editor(environments = [environment], workspace = false, workspaceVariables = [variable]) {
  return <I18nProvider initialLocale="en"><WorkspaceEnvironmentEditor environments={environments} environmentsLoading={false} mode={workspace ? {kind: "workspace"} : {kind: "environment", environmentId: "e"}} modeRevision={1} onCreate={vi.fn()} onDirtyChange={vi.fn()} onEnvironmentCreated={vi.fn()} onReplaceWorkspaceVariables={vi.fn()} onUpdate={vi.fn()} saving={false} workspaceVariables={workspaceVariables} workspaceVariablesLoading={false} /></I18nProvider>;
}
it("refreshes clean environment drafts after child-only changes", () => {
  const {rerender} = render(editor());
  rerender(editor([{...environment, variables: [{...environment.variables[0], value: "external", revision: 2}]}]));
  expect(screen.getByDisplayValue("external")).toBeTruthy();
});
it("keeps dirty environment drafts, blocks stale save and explicitly reloads", () => {
  const {rerender} = render(editor());
  fireEvent.change(screen.getByDisplayValue("old"), {target: {value: "local"}});
  rerender(editor([{...environment, variables: [{...environment.variables[0], value: "external", revision: 2}]}]));
  expect(screen.getByDisplayValue("local")).toBeTruthy();
  expect((screen.getByRole("button", {name: "Save"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", {name: "Load latest version"}));
  expect(screen.getByDisplayValue("external")).toBeTruthy();
});
it("retains a dirty draft after external deletion and removes clean deleted drafts", () => {
  const {rerender} = render(editor());
  fireEvent.change(screen.getByDisplayValue("old"), {target: {value: "local"}});
  rerender(editor([]));
  expect(screen.getByDisplayValue("local")).toBeTruthy();
  expect((screen.getByRole("button", {name: "Save"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", {name: "Load latest version"}));
  expect(screen.queryByDisplayValue("local")).toBeNull();
});
it("retains conflicting workspace variable edits until reload", () => {
  const {rerender} = render(editor([], true));
  fireEvent.change(screen.getByDisplayValue("old"), {target: {value: "local"}});
  rerender(editor([], true, [{...variable, value: "external", revision: 2}]));
  expect(screen.getByDisplayValue("local")).toBeTruthy();
  expect((screen.getByRole("button", {name: "Save"}) as HTMLButtonElement).disabled).toBe(true);
});

it("keeps edits typed after save submission and advances their baseline", async () => {
  let finish!: (value: WorkspaceEnvironment) => void;
  const update = vi.fn(() => new Promise<WorkspaceEnvironment>((resolve) => { finish = resolve; }));
  const view = (environments: WorkspaceEnvironment[]) => <I18nProvider initialLocale="en"><WorkspaceEnvironmentEditor environments={environments} environmentsLoading={false} mode={{kind: "environment", environmentId: "e"}} modeRevision={1} onCreate={vi.fn()} onDirtyChange={vi.fn()} onEnvironmentCreated={vi.fn()} onReplaceWorkspaceVariables={vi.fn()} onUpdate={update} saving={false} workspaceVariables={[]} workspaceVariablesLoading={false} /></I18nProvider>;
  const { rerender } = render(view([environment]));
  fireEvent.change(screen.getByDisplayValue("old"), {target: {value: "submitted"}});
  fireEvent.click(screen.getByRole("button", {name: "Save"}));
  fireEvent.change(screen.getByDisplayValue("submitted"), {target: {value: "later edit"}});
  const saved = {...environment, variables: [{...environment.variables[0], value: "submitted", revision: 2}]};
  await act(async () => { finish(saved); });
  rerender(view([saved]));
  expect(screen.getByDisplayValue("later edit")).toBeTruthy();
  expect(screen.getByRole("button", {name: "Save"})).not.toBeDisabled();
  expect(screen.queryByRole("alert")).toBeNull();
});

it("does not change selection when a pending creation finishes", async () => {
  let finish!: (value: WorkspaceEnvironment) => void;
  const create = vi.fn(() => new Promise<WorkspaceEnvironment>((resolve) => { finish = resolve; }));
  const created = vi.fn();
  const view = (mode: EnvironmentEditorMode, revision: number) => <I18nProvider initialLocale="en"><WorkspaceEnvironmentEditor environments={[environment]} environmentsLoading={false} mode={mode} modeRevision={revision} onCreate={create} onDirtyChange={vi.fn()} onEnvironmentCreated={created} onReplaceWorkspaceVariables={vi.fn()} onUpdate={vi.fn()} saving={false} workspaceVariables={[]} workspaceVariablesLoading={false} /></I18nProvider>;
  const { rerender } = render(view({kind: "new"}, 1));
  fireEvent.click(screen.getByRole("button", {name: "Save"}));
  rerender(view({kind: "workspace"}, 2));
  await act(async () => { finish({...environment, id: "new", variables: []}); });
  expect(screen.getByRole("heading", {name: "Workspace Variables"})).toBeTruthy();
  expect(created).not.toHaveBeenCalled();
});

it("keeps later edits to a newly created environment when selecting its saved record", async () => {
  let finish!: (value: WorkspaceEnvironment) => void;
  const create = vi.fn(() => new Promise<WorkspaceEnvironment>((resolve) => { finish = resolve; }));
  const created = vi.fn();
  const view = (environments: WorkspaceEnvironment[]) => <I18nProvider initialLocale="en"><WorkspaceEnvironmentEditor environments={environments} environmentsLoading={false} mode={{kind: "new"}} modeRevision={1} onCreate={create} onDirtyChange={vi.fn()} onEnvironmentCreated={created} onReplaceWorkspaceVariables={vi.fn()} onUpdate={vi.fn()} saving={false} workspaceVariables={[]} workspaceVariablesLoading={false} /></I18nProvider>;
  const { rerender } = render(view([]));
  fireEvent.click(screen.getByRole("button", {name: "Save"}));
  const name = screen.getByLabelText("Name");
  fireEvent.change(name, {target: {value: "later name"}});
  const saved = {...environment, id: "new", variables: []};
  await act(async () => { finish(saved); });
  rerender(view([saved]));
  expect(screen.getByDisplayValue("later name")).toBeTruthy();
  expect(created).toHaveBeenCalledWith("new", true);
  expect(screen.queryByRole("alert")).toBeNull();
});
