// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { I18nProvider } from "@unfour/ui";
import { StepEditor } from "./StepEditor";
import { InputEditor, RunInputs } from "./InputEditor";
import { newStep } from "./model";
import type { FlowInputDefinition, FlowStep } from "@unfour/command-client";
afterEach(cleanup);

function Editor({ initial, changed }: { initial: FlowStep; changed: (step: FlowStep) => void }) {
  const [step, setStep] = useState(initial);
  return <StepEditor step={step} after={[]} resources={{ api: [], database: [], ssh: [], connections: [] }} onRemove={() => {}} onValidity={() => {}} onChange={(value) => { setStep(value); changed(value); }} />;
}
it("edits canonical Wait Until policies and predicates with no mandatory attempt cap", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><Editor initial={newStep("waitUntil", "Ready")} changed={changed} /></I18nProvider>);
  expect(screen.getByLabelText("Check every (ms)")).toBeVisible();
  expect(screen.getByLabelText("Timeout (ms)")).toBeVisible();
  expect(screen.getByLabelText("Max attempts")).toHaveValue(null);
  fireEvent.change(screen.getByLabelText("Failure when (left / op / right, or null)"), { target: { value: '{"left":{"$ref":"/probe/status"},"op":"in","right":[400,404]}' } });
  fireEvent.change(screen.getByLabelText("Probe errors"), { target: { value: "retryTransientErrors" } });
  expect(changed.mock.lastCall?.[0]).toMatchObject({ kind: "waitUntil", maxAttempts: null, probeErrorPolicy: "retryTransientErrors", failureWhen: { op: "in", right: [400,404] } });
});
it("shows legacy Poll as Wait Until while retaining its node shape and attempt bound", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><Editor initial={newStep("poll", "Old poll")} changed={changed} /></I18nProvider>);
  expect(screen.getByText("Wait Until")).toBeInTheDocument();
  expect(screen.queryByLabelText("Probe errors")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Check every (ms)"), { target: { value: "2000" } });
  expect(changed.mock.lastCall?.[0]).toMatchObject({ kind: "poll", intervalMs: 2000, maxAttempts: 30, predicate: { op: "eq" } });
  expect(changed.mock.lastCall?.[0]).not.toHaveProperty("successWhen");
});
it("keeps partially typed secret JSON masked and clears errors after a valid value", () => {
  const valid = vi.fn();
  const changed = vi.fn();
  const definitions: FlowInputDefinition[] = [{ name: "payload", type: "json", required: true, secret: true }];
  render(<I18nProvider initialLocale="en"><RunInputs definitions={definitions} values={{}} onChange={changed} onValidity={valid} /></I18nProvider>);
  const field = screen.getByLabelText("payload");
  fireEvent.change(field, { target: { value: '{"a":' } });
  expect(field).toHaveAttribute("type", "password");
  expect(field).toHaveValue('{"a":');
  expect(valid).toHaveBeenLastCalledWith("payload", false);
  fireEvent.change(field, { target: { value: '{"a":1}' } });
  expect(changed).toHaveBeenLastCalledWith({ payload: { a: 1 } });
  expect(valid).toHaveBeenLastCalledWith("payload", true);
});
it("removes a default when an input becomes secret", () => {
  const changed = vi.fn();
  render(<I18nProvider initialLocale="en"><InputEditor definitions={[{ name: "value", type: "string", required: true, secret: false, default: "original" }]} onChange={changed} onValidity={() => {}} /></I18nProvider>);
  fireEvent.change(screen.getByLabelText("Secret"), { target: { value: "true" } });
  expect(changed.mock.lastCall?.[0]).toEqual([{ name: "value", type: "string", required: true, secret: true }]);
});

it("preserves an invalid JSON default when a different input is removed", () => {
  const validity = vi.fn();
  function Inputs() {
    const [definitions, setDefinitions] = useState<FlowInputDefinition[]>([
      { name: "first", type: "json", required: false, secret: false, default: {} },
      { name: "second", type: "json", required: false, secret: false, default: {} },
    ]);
    return <InputEditor definitions={definitions} onChange={setDefinitions} onValidity={validity} />;
  }
  render(<I18nProvider initialLocale="en"><Inputs /></I18nProvider>);
  fireEvent.change(screen.getAllByLabelText("Default (optional)")[1], { target: { value: "{" } });
  const invalidKey = validity.mock.lastCall?.[0];
  fireEvent.click(screen.getAllByRole("button", { name: "Remove input" })[0]);
  expect(screen.getByLabelText("Default (optional)")).toHaveValue("{");
  expect(validity.mock.calls.filter(([key]) => key === invalidKey).at(-1)).toEqual([invalidKey, false]);
});
