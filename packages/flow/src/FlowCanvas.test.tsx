// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { I18nProvider } from "@unfour/ui";
import type { FlowDefinition } from "@unfour/command-client";
import { FlowCanvas } from "./FlowCanvas";
import { newStep } from "./model";

afterEach(cleanup);

vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });

function definition(steps: FlowDefinition["steps"]): FlowDefinition {
  return { id: "flow", workspaceId: "ws", name: "Canvas", revision: 1, inputs: [], steps };
}

function Canvas({ initial }: { initial: FlowDefinition }) {
  const [draft, setDraft] = useState(initial);
  const [selected, setSelected] = useState<string | null>(null);
  return <I18nProvider initialLocale="en">
    <FlowCanvas definition={draft} selected={selected} onSelect={setSelected} onRemove={() => {}} onChange={setDraft} />
    <pre data-testid="steps">{JSON.stringify(draft.steps.map((step) => ({ id: step.id, kind: step.kind, next: "next" in step ? step.next : undefined, ifTrue: "ifTrue" in step ? step.ifTrue : undefined, ifFalse: "ifFalse" in step ? step.ifFalse : undefined })))}</pre>
  </I18nProvider>;
}

function steps() {
  return JSON.parse(screen.getByTestId("steps").textContent ?? "[]") as { id: string; kind: string; next?: string | null; ifTrue?: string; ifFalse?: string }[];
}

function insertOn(edgeId: string, kind: string) {
  fireEvent.change(screen.getByLabelText("Insert on connection"), { target: { value: edgeId } });
  fireEvent.change(screen.getByLabelText("Add step"), { target: { value: kind } });
}

it("keeps Add step inside Advanced instead of the toolbar", () => {
  render(<Canvas initial={definition([{ ...newStep("wait", "Wait"), id: "wait" }])} />);
  const toolbar = screen.getByRole("button", { name: "Zoom in" }).parentElement!;
  expect(within(toolbar).queryByLabelText("Add step")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Add step").closest("details")).not.toBeNull();
  expect(screen.getByLabelText("Insert on connection")).toBeInTheDocument();
});

it("inserts on an ordinary edge from the Advanced fallback", () => {
  render(<Canvas initial={definition([{ ...newStep("wait", "Wait"), id: "wait" }])} />);
  insertOn("wait:next", "api");
  const next = steps();
  expect(next.map((step) => step.kind)).toEqual(["wait", "action"]);
  expect(next[0].next).toBeNull();
  expect(next[1].next).toBe("$end");
});

it("inserts on Start from the Advanced fallback", () => {
  render(<Canvas initial={definition([{ ...newStep("wait", "Wait"), id: "wait" }])} />);
  insertOn("$start:next", "api");
  expect(steps().map((step) => step.kind)).toEqual(["action", "wait"]);
});

it("inserts independently on Condition true and false branches from Advanced", () => {
  render(<Canvas initial={definition([{ ...newStep("condition", "Condition"), id: "condition", ifTrue: "$end", ifFalse: "$end" }])} />);
  insertOn("condition:true", "wait");
  insertOn("condition:false", "waitUntil");
  const next = steps();
  expect(next.map((step) => step.kind)).toEqual(["condition", "waitUntil", "wait"]);
  expect(next[0]).toMatchObject({ id: "condition", ifTrue: next[2].id, ifFalse: next[1].id });
});
