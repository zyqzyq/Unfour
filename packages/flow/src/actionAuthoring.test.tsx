// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { I18nProvider } from "@unfour/ui";
import type { FlowAction, FlowDefinition, SshTaskDetail } from "@unfour/command-client";
import { sqlProblem } from "./actionAuthoring";
import { ApiActionEditor } from "./ApiActionEditor";
import { DatabaseActionEditor } from "./DatabaseActionEditor";
import { ActionTextField } from "./ActionTextField";
import { StepEditor } from "./StepEditor";
import { emptyAction, environmentInputErrors, resourceErrors, type Resources } from "./model";

afterEach(cleanup);
const detail = { task: { id: "task", name: "Deploy" }, localBinding: { defaultConnectionId: "default", lastUsedConnectionId: "last" }, detectedInputs: ["VERSION", "directory", "file"], steps: [
  { enabled: true, stepType: "command", configJson: { command: "echo {{VERSION}} {{VERSION}}", workingDirectory: "{{directory}}" } },
  { enabled: true, stepType: "upload", configJson: { localPath: "{{file}}", remotePath: "{{directory}}" } },
  { enabled: false, stepType: "command", configJson: { command: "{{disabled}}" } },
] } as SshTaskDetail;
const resources: Resources = { api: [{ id: "api", name: "Health", method: "GET", url: "https://example.test", bodyKind: "json", body: '{"ready":true}', headersJson: '[{"key":"Accept","value":"application/json","enabled":true}]' }], ssh: [{ id: "task", name: "Deploy", detail }], database: [{ id: "db", name: "Local" }], connections: [{ id: "default", name: "Default" }, { id: "last", name: "Last" }] };
const variables = [{ label: "version", path: ["inputs", "version"] }, { label: "Fetch · body", path: ["steps", "fetch", "body"] }];

it("uses the task engine's detected inputs instead of scanning templates again", () => {
  expect(detail.detectedInputs).toEqual(["VERSION", "directory", "file"]);
  const ignored = structuredClone(detail);
  ignored.steps[0].configJson = { command: "echo {{OTHER}}", workingDirectory: "", timeoutSeconds: 1, continueOnError: false };
  ignored.detectedInputs = ["VERSION"];
  expect(resourceErrors({ id: "flow", name: "Flow", workspaceId: "ws", revision: 1, inputs: [], steps: [{ id: "ssh", name: "SSH", kind: "action", timeoutMs: 1000, next: null, action: { ...emptyAction("ssh"), resourceId: "task", connectionId: "default", arguments: { inputs: { VERSION: "v1" } } } }] }, { ...resources, ssh: [{ id: "task", name: "Deploy", detail: ignored }] })).toEqual([]);
});
it("inserts escaped pointers at the selection without changing value type", () => {
  const changed = vi.fn();
  function Field() { const [value, setValue] = useState<unknown>("prefix suffix"); return <ActionTextField label="SQL" value={value} variables={variables} onValidity={() => {}} onChange={(v) => { setValue(v); changed(v); }} multiline />; }
  render(<I18nProvider initialLocale="en"><Field /></I18nProvider>);
  const field = screen.getByLabelText("SQL") as HTMLTextAreaElement;
  field.setSelectionRange(7, 7); fireEvent.select(field);
  fireEvent.click(screen.getByText("Insert variable", { selector: "summary" }));
  fireEvent.change(screen.getByLabelText("SQL · Variable"), { target: { value: "/steps/fetch/body" } });
  fireEvent.change(screen.getByLabelText("SQL · Nested field (optional, dot separated)"), { target: { value: "items.0.id" } });
  fireEvent.click(screen.getByRole("button", { name: "Insert variable" }));
  expect(changed).toHaveBeenLastCalledWith("prefix ${/steps/fetch/body/items/0/id}suffix");
  expect(screen.queryByLabelText("SQL · Type")).not.toBeInTheDocument();
});
it("shows inherited saved request data without writing overrides on mount", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><ApiActionEditor action={{ ...emptyAction(), resourceId: "api" }} resources={resources} variables={variables} onChange={changed} onValidity={() => {}} /></I18nProvider>);
  expect(screen.getByText("GET · Health")).toBeVisible();
  expect(screen.getByText(/Accept: application\/json/)).toBeVisible();
  expect(screen.getByText('{"ready":true}')).toBeVisible();
  expect(changed).not.toHaveBeenCalled();
});
it("edits a legacy replacement without migrating to a patch", () => {
  const changed = vi.fn();
  const action = { ...emptyAction(), resourceId: "api", arguments: { headers: [] } };
  render(<I18nProvider initialLocale="en"><ApiActionEditor action={action} resources={resources} variables={[]} onChange={changed} onValidity={() => {}} /></I18nProvider>);
  fireEvent.click(within(screen.getByRole("group", { name: "Headers" })).getByRole("button", { name: "Add override" }));
  expect(changed.mock.lastCall?.[0].arguments).toEqual({ headers: [{ key: "", value: "", enabled: true }] });
});
it("fixes the default connection on first task selection and exposes detected bindings", () => {
  const changed = vi.fn();
  function Editor() {
    const [action, setAction] = useState(emptyAction("ssh"));
    return <StepEditor step={{ id: "ssh", name: "SSH", kind: "action", timeoutMs: 1000, next: null, action }} after={[]} resources={resources} onRemove={() => {}} onValidity={() => {}} onChange={(step) => { if (step.kind === "action") { setAction(step.action); changed(step.action); } }} />;
  }
  render(<I18nProvider initialLocale="en"><Editor /></I18nProvider>);
  fireEvent.change(screen.getByLabelText("Referenced resource"), { target: { value: "task" } });
  expect(changed).toHaveBeenLastCalledWith({ capability: "ssh", resourceId: "task", connectionId: "default", arguments: { inputs: {}, workspaceDefaults: true } });
  expect(screen.getByLabelText("VERSION")).toBeVisible();
  expect(screen.queryByText("Add field")).not.toBeInTheDocument();
});
it("validates SSH defaults per selected environment while preserving explicit and legacy bindings", () => {
  const action: FlowAction = { ...emptyAction("ssh"), resourceId: "task", connectionId: "default", arguments: { inputs: { directory: "/tmp", file: "${/inputs/file}" }, workspaceDefaults: true } };
  const flow: FlowDefinition = { id: "flow", name: "Flow", workspaceId: "ws", revision: 1, inputs: [], steps: [{ id: "ssh", name: "SSH", kind: "action", timeoutMs: 1000, next: null, action }] };
  expect(resourceErrors(flow, resources)).toEqual([]);
  expect(environmentInputErrors(flow, resources, new Map())).toContainEqual({ name: "SSH", key: "flow.sshMissingInputs" });
  expect(environmentInputErrors(flow, resources, new Map([["version", "v1"]]))).toEqual([]);
  action.arguments.workspaceDefaults = false;
  expect(resourceErrors(flow, resources)).toHaveLength(1);
  expect(environmentInputErrors(flow, resources, new Map([["version", "v1"]]))).toEqual([]);
});
it("does not fall back to lastUsedConnectionId or mutate an existing SSH action on mount", () => {
  const changed = vi.fn();
  const withoutDefault = structuredClone(resources);
  withoutDefault.ssh[0].detail!.localBinding!.defaultConnectionId = null;
  render(<I18nProvider initialLocale="en"><StepEditor step={{ id: "ssh", name: "SSH", kind: "action", timeoutMs: 1000, next: null, action: emptyAction("ssh") }} after={[]} resources={withoutDefault} onRemove={() => {}} onValidity={() => {}} onChange={changed} /></I18nProvider>);
  expect(changed).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Referenced resource"), { target: { value: "task" } });
  expect(changed.mock.lastCall?.[0].action.connectionId).toBeNull();
});
it("keeps SQL visible and required without a Remove control", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><DatabaseActionEditor action={{ ...emptyAction("database"), arguments: {} }} resources={resources} variables={variables} onChange={changed} onValidity={() => {}} /></I18nProvider>);
  expect(screen.getByLabelText("SQL").tagName).toBe("TEXTAREA");
  expect(screen.getByRole("alert")).toHaveTextContent("SQL is required");
  expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
});
it.each([undefined, "", "  ", "-- comment\n;", "/* comment */"])("rejects empty SQL %s", (sql) => expect(sqlProblem(sql)).toBe("flow.sqlRequired"));
it.each(["SELECT ';'; -- comment", "SELECT ${/inputs/value};", "SELECT $$a;b$$", "SELECT 'it''s;ok';"])("accepts one statement %s", (sql) => expect(sqlProblem(sql)).toBeNull());
it("rejects multiple SQL statements", () => expect(sqlProblem("SELECT 1; /* separator */ SELECT 2")).toBe("flow.sqlSingleStatement"));
