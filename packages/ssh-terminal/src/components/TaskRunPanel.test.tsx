// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { TaskRunPanel } from "./TaskRunPanel";

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });
const props: ComponentProps<typeof TaskRunPanel> = {
  cancelling: false, events: [], onCancel: () => {}, onClose: () => {},
  run: { id: "run", workspaceId: "ws", taskId: "task", connectionId: "conn", status: "running", startedAt: "", finishedAt: null, errorMessage: null, logPath: "" },
  task: { task: { id: "task", workspaceId: "ws", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null }, steps: [], localBinding: null, detectedInputs: [] },
};

it("offers rerun after a terminal completion event even before history polling updates the run", () => {
  const onRerun = vi.fn();
  const { rerender } = render(<TaskRunPanel {...props} onRerun={onRerun} />);
  expect(screen.queryByRole("button", { name: "Run again" })).not.toBeInTheDocument();
  rerender(<TaskRunPanel {...props} onRerun={onRerun} events={[{
    runId: "run", taskId: "task", kind: "run", status: "cancelled", stepId: null,
    stepName: null, stepType: null, position: null, stream: null, data: null,
    exitCode: null, durationMs: null, direction: null, transferredBytes: null,
    totalBytes: null, bytesPerSecond: null, error: null, createdAt: "now",
  }]} />);
  fireEvent.click(screen.getByRole("button", { name: "Run again" }));
  expect(onRerun).toHaveBeenCalledOnce();
});

it.each(["pointerUp", "pointerCancel", "unmount"])("removes resize listeners on %s without cancelling the SSH run", (finish) => {
  const add = vi.spyOn(window, "addEventListener");
  const remove = vi.spyOn(window, "removeEventListener");
  const cancel = vi.fn();
  const { unmount } = render(<TaskRunPanel {...props} onCancel={cancel} />);
  fireEvent.pointerDown(screen.getByRole("separator"), { clientY: 400 });
  const registrations = add.mock.calls.filter(([type]) => ["pointermove", "pointerup", "pointercancel"].includes(type));
  expect(registrations).toHaveLength(3);
  if (finish === "unmount") unmount();
  else if (finish === "pointerUp") fireEvent.pointerUp(window);
  else fireEvent.pointerCancel(window);
  for (const [type, listener] of registrations) expect(remove).toHaveBeenCalledWith(type, listener);
  expect(cancel).not.toHaveBeenCalled();
});
