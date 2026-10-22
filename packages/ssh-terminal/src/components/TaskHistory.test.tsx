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
  const props = { clearing: false, failed: true, loading: false, onClear: vi.fn(), onSelectRun: vi.fn(), onRetry, retrying: false, runs: [run] };
  const { rerender } = render(<TaskHistory {...props} />);
  expect(screen.getByRole("alert")).toHaveTextContent("ssh.tasks.history.loadFailed");
  expect(screen.getByText("Connection refused")).toBeInTheDocument();
  expect(screen.queryByText("ssh.tasks.history.empty")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.actions.retry" }));
  expect(onRetry).toHaveBeenCalledOnce();

  rerender(<TaskHistory {...props} retrying />);
  const retry = screen.getByRole("button", { name: "ssh.tasks.actions.retry" });
  expect(retry).toBeDisabled();
  fireEvent.click(retry);
  expect(onRetry).toHaveBeenCalledOnce();
  expect(screen.getByRole("alert")).toBeInTheDocument();
  expect(screen.getByRole("table")).toHaveTextContent("Connection refused");

  rerender(<TaskHistory {...props} failed={false} />);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByRole("table")).toHaveTextContent("Connection refused");
});

it("shows a full load error without cached history and keeps retry available", () => {
  const onRetry = vi.fn();
  const props = { clearing: false, failed: true, loading: false, onClear: vi.fn(), onSelectRun: vi.fn(), onRetry, retrying: false, runs: [] };
  const { rerender } = render(<TaskHistory {...props} />);
  expect(screen.getByRole("alert")).toHaveTextContent("ssh.tasks.history.loadFailed");
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(screen.queryByText("ssh.tasks.history.empty")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "ssh.tasks.history.clear" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.actions.retry" }));
  expect(onRetry).toHaveBeenCalledOnce();

  rerender(<TaskHistory {...props} retrying />);
  expect(screen.getByRole("button", { name: "ssh.tasks.actions.retry" })).toBeDisabled();
  expect(screen.getByRole("alert")).toBeInTheDocument();
});
