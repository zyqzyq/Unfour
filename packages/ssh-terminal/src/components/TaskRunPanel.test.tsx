// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { I18nProvider } from "@unfour/ui";
import { TaskRunPanel } from "./TaskRunPanel";

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });
const props: ComponentProps<typeof TaskRunPanel> = {
  cancelling: false, events: [], onCancel: () => {}, onClose: () => {},
  run: { id: "run", workspaceId: "ws", taskId: "task", connectionId: "conn", status: "running", startedAt: "", finishedAt: null, errorMessage: null, logPath: "" },
  task: { task: { id: "task", workspaceId: "ws", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null }, steps: [], localBinding: null, detectedInputs: [] },
};

it.each(["failed", "cancelled"] as const)("shows a localized unknown outcome for %s completion and after reopening the run", (status) => {
  const error = "SSH_TASK_OUTCOME_UNCERTAIN";
  const finalEvent = {
    runId: "run", taskId: "task", kind: "run", status, stepId: null,
    stepName: null, stepType: null, position: null, stream: null, data: null,
    exitCode: null, durationMs: null, direction: null, transferredBytes: null,
    totalBytes: null, bytesPerSecond: null, error, createdAt: "now",
  } as const;
  const { rerender } = render(<I18nProvider initialLocale="zh-CN"><TaskRunPanel {...props}
    events={[{ ...finalEvent, kind: "step", stepId: "command" }, finalEvent]} /></I18nProvider>);
  expect(screen.getByText("结果未知")).toBeVisible();
  expect(screen.getByText(/↳ 结果未知/)).toBeVisible();
  expect(screen.queryByText(error)).not.toBeInTheDocument();

  rerender(<I18nProvider initialLocale="zh-CN"><TaskRunPanel {...props}
    run={{ ...props.run, status, errorMessage: error }} /></I18nProvider>);
  expect(screen.getByText("结果未知")).toBeVisible();
  expect(screen.getByText(/命令已发送/)).toBeVisible();
});

it("pauses output follow while reading, resumes explicitly, and resets for a different run", () => {
  const completed = { ...props.run, status: "success" as const };
  const { rerender } = render(<TaskRunPanel {...props} run={completed} logText="first log" />);
  const output = screen.getByLabelText("Task output");
  Object.defineProperties(output, { scrollHeight: { configurable: true, value: 1000 }, clientHeight: { configurable: true, value: 200 } });
  output.scrollTop = 100;
  fireEvent.scroll(output);
  rerender(<TaskRunPanel {...props} run={completed} logText="additional log" />);
  expect(output.scrollTop).toBe(100);
  fireEvent.click(screen.getByRole("button", { name: "Follow latest output" }));
  expect(output.scrollTop).toBe(1000);
  expect(output).toHaveFocus();
  output.scrollTop = 100;
  fireEvent.scroll(output);
  rerender(<TaskRunPanel {...props} run={{ ...completed, id: "other-run" }} logText="different run" />);
  expect(output.scrollTop).toBe(1000);
  expect(screen.queryByRole("button", { name: "Follow latest output" })).not.toBeInTheDocument();
});

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
  expect(screen.getByText("Cancelled")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Run again" }));
  expect(onRerun).toHaveBeenCalledOnce();
});

it("resets follow when returning from a short run that emitted no scroll event", () => {
  const completed = { ...props.run, status: "success" as const };
  const { rerender } = render(<TaskRunPanel {...props} run={completed} logText="long log" />);
  const output = screen.getByLabelText("Task output");
  Object.defineProperties(output, { scrollHeight: { configurable: true, value: 1000 }, clientHeight: { configurable: true, value: 200 } });
  output.scrollTop = 0;
  fireEvent.scroll(output);
  expect(screen.getByRole("button", { name: "Follow latest output" })).toBeInTheDocument();
  // A short log cannot scroll, so no browser scroll event updates the run ID.
  Object.defineProperty(output, "scrollHeight", { configurable: true, value: 200 });
  rerender(<TaskRunPanel {...props} run={{ ...completed, id: "short-run" }} logText="short log" />);
  Object.defineProperty(output, "scrollHeight", { configurable: true, value: 1000 });
  rerender(<TaskRunPanel {...props} run={completed} logText="long log" />);
  expect(screen.queryByRole("button", { name: "Follow latest output" })).not.toBeInTheDocument();
  expect(output.scrollTop).toBe(1000);
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
