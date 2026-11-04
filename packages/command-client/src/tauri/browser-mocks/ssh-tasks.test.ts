import { beforeEach, describe, expect, it } from "vitest";
import type { SshTask, SshTaskDetail, SshTaskSaveInput } from "../../types";
import { handleSshTaskMock } from "./ssh-tasks";
import { mockStore } from "./state";

describe("SSH task browser mocks", () => {
  beforeEach(() => {
    mockStore.sshTasks = [];
    mockStore.sshTaskRuns = [];
  });

  it("appends new tasks and persists an exact manual reorder", () => {
    const first = saveTask("First");
    const second = saveTask("Second");
    expect([first.task.sortOrder, second.task.sortOrder]).toEqual([0, 1]);

    const reordered = handleSshTaskMock<SshTask[]>("ssh_tasks_reorder", {
      input: {
        workspaceId: "workspace",
        taskIds: [second.task.id, first.task.id],
      },
    }) as SshTask[];
    expect(reordered.map(({ id, sortOrder }) => [id, sortOrder])).toEqual([
      [second.task.id, 0],
      [first.task.id, 1],
    ]);
    expect(reordered.map(({ updatedAt }) => updatedAt)).toEqual([
      second.task.updatedAt,
      first.task.updatedAt,
    ]);
    expect(
      (handleSshTaskMock<SshTask[]>("ssh_tasks_list", {
        workspaceId: "workspace",
      }) as SshTask[]).map(({ id }) => id),
    ).toEqual([second.task.id, first.task.id]);
  });

  it("rejects incomplete manual reorder payloads", () => {
    const first = saveTask("First");
    saveTask("Second");
    expect(() =>
      handleSshTaskMock("ssh_tasks_reorder", {
        input: { workspaceId: "workspace", taskIds: [first.task.id] },
      }),
    ).toThrow("every active task");
  });

  it("rejects a stale editor snapshot after a task changes or is deleted", () => {
    const original = saveTask("Original");
    const input = { id: original.task.id, workspaceId: "workspace", name: "External", description: "", steps: [] };
    handleSshTaskMock("ssh_task_save", { input });
    expect(() => handleSshTaskMock("ssh_task_save", { input: { ...input, name: "Draft" }, expectedSnapshot: original })).toThrow();
    expect(mockStore.sshTasks[0].task.name).toBe("External");
    handleSshTaskMock("ssh_task_delete", { workspaceId: "workspace", taskId: original.task.id });
    expect(() => handleSshTaskMock("ssh_task_save", { input, expectedSnapshot: original })).toThrow();
  });
});

function saveTask(name: string): SshTaskDetail {
  return handleSshTaskMock<SshTaskDetail>("ssh_task_save", {
    input: {
      workspaceId: "workspace",
      name,
      description: "",
      defaultConnectionId: null,
      steps: [],
    } satisfies SshTaskSaveInput,
  }) as SshTaskDetail;
}
