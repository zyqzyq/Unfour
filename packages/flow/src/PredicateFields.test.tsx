// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { I18nProvider } from "@unfour/ui";
import type { FlowPredicate } from "@unfour/command-client";
import { PredicateFields } from "./StructuredFields";
import { ActionTextField } from "./ActionTextField";
afterEach(cleanup);

const variables = [{ label: "status", path: ["inputs", "status"] }, { label: "Fetch · body", path: ["steps", "fetch", "body"] }, { label: "body", path: ["probe", "body"] }];
function Builder({ initial, changed }: { initial: FlowPredicate; changed: (value: FlowPredicate | null) => void }) {
  const [value, setValue] = useState<FlowPredicate | null>(initial);
  return <I18nProvider initialLocale="en"><PredicateFields label="Until" value={value} variables={variables} onValidity={() => {}} onChange={(next) => { setValue(next); changed(next); }} /></I18nProvider>;
}

it("edits nested probe fields, literal/variable RHS and boolean shortcuts", () => {
  const changed = vi.fn();
  render(<Builder initial={{ left: { $ref: "/probe/body/status" }, op: "eq", right: "ready" }} changed={changed} />);
  expect(changed).not.toHaveBeenCalled();
  const source = screen.getByLabelText("Until · Value · Variable");
  expect(within(source).getByRole("group", { name: "Flow Inputs" })).toBeInTheDocument();
  expect(within(source).getByRole("group", { name: "Previous Steps" })).toBeInTheDocument();
  expect(within(source).getByRole("group", { name: "Poll / Wait Until probe output" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Nested field (optional, dot separated)"), { target: { value: "result.status" } });
  expect(changed).toHaveBeenLastCalledWith({ left: { $ref: "/probe/body/result/status" }, op: "eq", right: "ready" });
  fireEvent.change(screen.getByLabelText("Until · Compare with · Type"), { target: { value: "variable" } });
  fireEvent.change(screen.getByLabelText("Until · Compare with · Variable"), { target: { value: "/inputs/status" } });
  expect(changed.mock.lastCall?.[0].right).toEqual({ $ref: "/inputs/status" });
  fireEvent.change(screen.getByLabelText("Until · Operator"), { target: { value: "false" } });
  expect(changed.mock.lastCall?.[0]).toMatchObject({ op: "eq", right: false });
  expect(screen.queryByLabelText("Until · Compare with · Type")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Until · Operator"), { target: { value: "eq" } });
  fireEvent.change(screen.getByLabelText("Until · Compare with"), { target: { value: "true" } });
  expect(screen.getByLabelText("Until · Operator")).toHaveValue("eq");
  expect(changed.mock.lastCall?.[0].right).toBe(true);
  expect(within(screen.getByLabelText("Until · Operator")).getByRole("option", { name: "Exists (unsupported)" })).toBeDisabled();
});

it("loads legacy in as contains and edits without changing wire operand meaning", () => {
  const changed = vi.fn();
  render(<Builder initial={{ left: { $ref: "/probe/status" }, op: "in", right: [200, 204] }} changed={changed} />);
  expect(changed).not.toHaveBeenCalled();
  expect(screen.getByLabelText("Until · Operator")).toHaveValue("contains");
  fireEvent.change(screen.getByLabelText("Until · Value", { exact: true }), { target: { value: "[200,202]" } });
  expect(changed).toHaveBeenLastCalledWith({ left: { $ref: "/probe/status" }, op: "in", right: [200, 202] });
});

it("inserts workspace/environment API templates without inventing Flow refs or storing values", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><ActionTextField label="URL" value="/status" variables={[...variables, { label: "base_url", path: ["environment", "base_url"] }]} onValidity={() => {}} onChange={changed} /></I18nProvider>);
  fireEvent.click(screen.getByText("Insert variable", { selector: "summary" }));
  fireEvent.change(screen.getByLabelText("URL · Variable"), { target: { value: "/environment/base_url" } });
  expect(screen.getByLabelText("URL · Nested field (optional, dot separated)")).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Insert variable" }));
  expect(changed).toHaveBeenCalledWith("{{base_url}}/status");
});
