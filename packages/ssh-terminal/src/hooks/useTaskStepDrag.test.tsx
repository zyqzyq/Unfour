// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SshTaskSaveInput } from "@unfour/command-client";
import { createTaskStep } from "../model/task-template";
import { useTaskStepDrag } from "./useTaskStepDrag";

const draft: SshTaskSaveInput = {
  workspaceId: "ws", name: "Task", description: "", defaultConnectionId: null,
  steps: [{ ...createTaskStep("command", 0), id: "a" }, { ...createTaskStep("command", 1), id: "b" }],
};
const originalHitTest = Object.getOwnPropertyDescriptor(document, "elementFromPoint");
beforeEach(() => {
  vi.stubGlobal("PointerEvent", MouseEvent);
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: () => screen.getByText("second") });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  if (originalHitTest) Object.defineProperty(document, "elementFromPoint", originalHitTest);
  else Reflect.deleteProperty(document, "elementFromPoint");
});

function Probe({ value, onChange }: { value: SshTaskSaveInput; onChange: (value: SshTaskSaveInput) => void }) {
  const { onStepDragHandlePointerDown } = useTaskStepDrag(value, onChange, () => {});
  return <>
    <span data-step-index="0" onPointerDown={(event) => onStepDragHandlePointerDown(0, event)}>first</span>
    <span data-step-index="1">second</span>
  </>;
}

function startDrag() {
  const handle = screen.getByText("first");
  handle.setPointerCapture = vi.fn();
  handle.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(handle, { button: 0 });
  fireEvent.pointerMove(window, { clientX: 10, clientY: 10 });
}

it("finishes a drag against the latest committed draft and callback only once", () => {
  const oldChange = vi.fn();
  const change = vi.fn();
  const { rerender } = render(<Probe value={draft} onChange={oldChange} />);
  startDrag();
  rerender(<Probe value={{ ...draft, name: "Edited during drag" }} onChange={change} />);
  fireEvent.pointerUp(window);
  expect(oldChange).not.toHaveBeenCalled();
  expect(change).toHaveBeenCalledTimes(1);
  expect(change.mock.calls[0][0].name).toBe("Edited during drag");
  expect(change.mock.calls[0][0].steps.map((step: { id: string }) => step.id)).toEqual(["b", "a"]);
  fireEvent.pointerUp(window);
  expect(change).toHaveBeenCalledTimes(1);
});

it("removes all drag listeners on unmount without committing a reorder", () => {
  const add = vi.spyOn(window, "addEventListener");
  const remove = vi.spyOn(window, "removeEventListener");
  const change = vi.fn();
  const { unmount } = render(<Probe value={draft} onChange={change} />);
  startDrag();
  unmount();
  for (const [type, listener] of add.mock.calls.filter(([type]) => ["pointermove", "pointerup", "pointercancel"].includes(type))) {
    expect(remove).toHaveBeenCalledWith(type, listener);
  }
  fireEvent.pointerUp(window);
  expect(change).not.toHaveBeenCalled();
});
