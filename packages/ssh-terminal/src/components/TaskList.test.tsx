// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
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
});

function renderTaskList({
  onNew = vi.fn(),
  onOpenConnections = vi.fn(),
}: {
  onNew?: () => void;
  onOpenConnections?: () => void;
}) {
  return render(
    <TaskList
      loading={false}
      onDelete={vi.fn()}
      onDuplicate={vi.fn()}
      onExample={vi.fn()}
      onNew={onNew}
      onOpenConnections={onOpenConnections}
      onRun={vi.fn()}
      onSelect={vi.fn()}
      selectedTaskId={null}
      tasks={[]}
    />,
  );
}
