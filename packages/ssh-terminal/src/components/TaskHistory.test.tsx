// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { SshTaskRun } from "@unfour/command-client";
import { TaskHistory } from "./TaskHistory";

vi.mock("@unfour/ui", async (original) => ({
  ...await original<typeof import("@unfour/ui")>(), useI18n: () => ({ t: (key: string) => key }),
}));
afterEach(cleanup);

it("offers separate rerun actions for every completed status without selecting history", () => {
  const onRerun = vi.fn();
  const onSelectRun = vi.fn();
  const runs: SshTaskRun[] = (["failed", "success", "cancelled", "running"] as const).map((status) => ({
    id: status, taskId: "task", workspaceId: "workspace", connectionId: null, status,
    startedAt: "2026-01-01T00:00:00Z", finishedAt: null, errorMessage: null, logPath: "",
  }));
  render(<TaskHistory clearing={false} failed={false} loading={false} onClear={vi.fn()} onSelectRun={onSelectRun} onRerun={onRerun} onRetry={vi.fn()} retrying={false} runs={runs} />);
  const buttons = screen.getAllByRole("button", { name: "ssh.tasks.actions.rerun" });
  expect(buttons).toHaveLength(3);
  buttons.forEach((button) => { fireEvent.keyDown(button, { key: "Enter" }); fireEvent.click(button); });
  expect(onRerun.mock.calls.map(([run]) => run.status)).toEqual(["failed", "success", "cancelled"]);
  expect(onSelectRun).not.toHaveBeenCalled();
});

it("keeps cached history visible when a refresh fails and offers retry", () => {
  const onRetry = vi.fn();
  const run: SshTaskRun = {
    id: "run", taskId: "task", workspaceId: "workspace", connectionId: null, status: "failed",
    startedAt: "2026-01-01T00:00:00Z", finishedAt: null, errorMessage: "Connection refused", logPath: "",
  };
  render(<TaskHistory clearing={false} failed loading={false} onClear={vi.fn()} onSelectRun={vi.fn()} onRetry={onRetry} retrying={false} runs={[run]} />);
  expect(screen.getByRole("alert")).toHaveTextContent("ssh.tasks.history.loadFailed");
  expect(screen.getByText("Connection refused")).toBeInTheDocument();
  expect(screen.queryByText("ssh.tasks.history.empty")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.actions.retry" }));
  expect(onRetry).toHaveBeenCalledOnce();
});
