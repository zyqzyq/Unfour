// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { TaskRunPanel } from "./TaskRunPanel";

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });
const props: ComponentProps<typeof TaskRunPanel> = {
  cancelling: false, events: [], onCancel: () => {}, onClose: () => {},
  run: { id: "run", workspaceId: "ws", taskId: "task", connectionId: "conn", status: "running", startedAt: "", finishedAt: null, errorMessage: null, logPath: "" },
  task: { task: { id: "task", workspaceId: "ws", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null }, steps: [], localBinding: null },
};

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
