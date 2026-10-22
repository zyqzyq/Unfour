// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SshTask } from "@unfour/command-client";
import { reorderTaskIds, sortTasksForView } from "../model/task-list-order";
import { TaskList } from "./TaskList";

vi.mock("@unfour/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@unfour/ui")>();
  return {
    ...actual,
    useI18n: () => ({ t: (key: string) => key }),
  };
});

afterEach(cleanup);

describe("TaskList", () => {
  it("invokes new-task actions without forwarding the click event as a template", () => {
    const onNew = vi.fn();
    renderTaskList({ onNew });

    const newTaskButtons = screen.getAllByRole("button", {
      name: "ssh.tasks.actions.new",
    });
    expect(newTaskButtons).toHaveLength(2);

    newTaskButtons.forEach((button) => fireEvent.click(button));

    expect(onNew.mock.calls).toEqual([[], []]);
  });

  it("switches back to the connections sidebar", () => {
    const onOpenConnections = vi.fn();
    renderTaskList({ onOpenConnections });

    fireEvent.click(
      screen.getByRole("tab", { name: "ssh.homeTabs.connections" }),
    );

    expect(onOpenConnections).toHaveBeenCalledOnce();
  });

  it("keeps cached tasks visible when a refresh fails", () => {
    const onRetry = vi.fn();
    const { rerender } = renderTaskList({ failed: true, onRetry, tasks: [task()] });
    expect(screen.getByRole("alert")).toHaveTextContent("ssh.tasks.list.loadFailed");
    expect(screen.getByRole("button", { name: "Task" })).toBeInTheDocument();
    expect(screen.queryByText("ssh.tasks.list.emptyTitle")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.actions.retry" }));
    expect(onRetry).toHaveBeenCalledOnce();

    rerender(taskList({ failed: true, onRetry, retrying: true, tasks: [task()] }));
    const retry = screen.getByRole("button", { name: "ssh.tasks.actions.retry" });
    expect(retry).toBeDisabled();
    fireEvent.click(retry);
    expect(onRetry).toHaveBeenCalledOnce();
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Task" })).toBeInTheDocument();

    rerender(taskList({ tasks: [task()] }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Task" })).toBeInTheDocument();
  });

  it("shows a full load error without cached tasks and keeps retry available", () => {
    const onRetry = vi.fn();
    const { rerender } = renderTaskList({ failed: true, onRetry });
    expect(screen.getByRole("alert")).toHaveTextContent("ssh.tasks.list.loadFailed");
    expect(screen.queryByRole("tree")).not.toBeInTheDocument();
    expect(screen.queryByText("ssh.tasks.list.emptyTitle")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.actions.retry" }));
    expect(onRetry).toHaveBeenCalledOnce();

    rerender(taskList({ failed: true, retrying: true }));
    expect(screen.getByRole("button", { name: "ssh.tasks.actions.retry" })).toBeDisabled();
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("keeps refresh errors alongside the filter when no cached task matches", () => {
    renderTaskList({ failed: true, tasks: [task()] });
    fireEvent.change(screen.getByRole("textbox", { name: "ssh.tasks.list.filter" }), {
      target: { value: "missing" },
    });
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("ssh.tasks.list.filterEmpty")).toBeInTheDocument();
    expect(screen.queryByText("ssh.tasks.list.emptyTitle")).not.toBeInTheDocument();
  });

  it("sorts tasks by manual position, name, and last update", () => {
    const tasks = [
      task({ id: "beta", name: "Beta", sortOrder: 0, updatedAt: "2026-01-01" }),
      task({ id: "alpha", name: "Alpha", sortOrder: 1, updatedAt: "2026-02-01" }),
    ];
    expect(sortTasksForView(tasks, "manual").map(({ id }) => id)).toEqual([
      "beta",
      "alpha",
    ]);
    expect(sortTasksForView(tasks, "name").map(({ id }) => id)).toEqual([
      "alpha",
      "beta",
    ]);
    expect(sortTasksForView(tasks, "updated").map(({ id }) => id)).toEqual([
      "alpha",
      "beta",
    ]);
  });

  it("calculates before and after drag orders", () => {
    expect(reorderTaskIds(["a", "b", "c"], "c", "a", "before")).toEqual([
      "c",
      "a",
      "b",
    ]);
    expect(reorderTaskIds(["a", "b", "c"], "a", "b", "after")).toEqual([
      "b",
      "a",
      "c",
    ]);
  });

  it("disables dragging while a task filter is active", () => {
    renderTaskList({ tasks: [task({ id: "one", name: "One" })] });
    const taskButton = screen.getByRole("button", { name: "One" });
    expect(taskButton).toHaveClass("cursor-grab");
    fireEvent.change(screen.getByRole("textbox", { name: "ssh.tasks.list.filter" }), {
      target: { value: "One" },
    });
    expect(taskButton).not.toHaveClass("cursor-grab");
  });
});

function renderTaskList(props: Parameters<typeof taskList>[0]) {
  return render(taskList(props));
}

function taskList({
  failed = false,
  onNew = vi.fn(),
  onOpenConnections = vi.fn(),
  onRetry = vi.fn(),
  retrying = false,
  tasks = [],
}: {
  failed?: boolean;
  onNew?: () => void;
  onOpenConnections?: () => void;
  onRetry?: () => void;
  retrying?: boolean;
  tasks?: SshTask[];
}) {
  return (
    <TaskList
      failed={failed}
      loading={false}
      onDelete={vi.fn()}
      onDuplicate={vi.fn()}
      onExample={vi.fn()}
      onNew={onNew}
      onOpenConnections={onOpenConnections}
      onReorder={vi.fn()}
      onRun={vi.fn()}
      onRetry={onRetry}
      onSelect={vi.fn()}
      reordering={false}
      retrying={retrying}
      selectedTaskId={null}
      tasks={tasks}
    />
  );
}

function task(overrides: Partial<SshTask> = {}): SshTask {
  return {
    id: "task",
    workspaceId: "workspace",
    name: "Task",
    description: "",
    sortOrder: 0,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
    deletedAt: null,
    ...overrides,
  };
}
