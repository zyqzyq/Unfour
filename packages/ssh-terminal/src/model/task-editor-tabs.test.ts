import { describe, expect, it } from "vitest";
import type { SshTaskSaveInput } from "@unfour/command-client";
import {
  closeTaskTab,
  createEmptyTaskEditorState,
  hydrateTaskTab,
  isTaskTabDirty,
  openNewTaskTab,
  openSavedTaskTab,
  persistTaskTab,
  updateTaskTabDraft,
} from "./task-editor-tabs";

describe("SSH task editor tabs", () => {
  it("opens each saved task in an independent tab and preserves its draft", () => {
    let state = openSavedTaskTab(createEmptyTaskEditorState(), "task-a");
    state = hydrateTaskTab(state, "task:task-a", draft("task-a", "Task A"));
    state = updateTaskTabDraft(
      state,
      "task:task-a",
      draft("task-a", "Task A edited"),
    );
    state = openSavedTaskTab(state, "task-b");
    state = hydrateTaskTab(state, "task:task-b", draft("task-b", "Task B"));

    expect(state.tabs).toHaveLength(2);
    expect(state.tabs[0].draft?.name).toBe("Task A edited");
    expect(state.tabs[1].draft?.name).toBe("Task B");
    expect(state.activeTabId).toBe("task:task-b");
  });

  it("rekeys a new tab after the task is first saved", () => {
    let state = openNewTaskTab(
      createEmptyTaskEditorState(),
      "new:1",
      draft(undefined, "New Task"),
    );

    expect(isTaskTabDirty(state.tabs[0])).toBe(true);
    state = persistTaskTab(state, "new:1", draft("saved-task", "New Task"));

    expect(state.activeTabId).toBe("task:saved-task");
    expect(state.tabs[0].taskId).toBe("saved-task");
    expect(isTaskTabDirty(state.tabs[0])).toBe(false);
  });

  it("selects the adjacent tab when the active tab closes", () => {
    let state = openSavedTaskTab(createEmptyTaskEditorState(), "task-a");
    state = openSavedTaskTab(state, "task-b");
    state = openSavedTaskTab(state, "task-c");
    state = { ...state, activeTabId: "task:task-b" };

    state = closeTaskTab(state, "task:task-b");

    expect(state.tabs.map((tab) => tab.taskId)).toEqual(["task-a", "task-c"]);
    expect(state.activeTabId).toBe("task:task-c");
  });
});

function draft(id: string | undefined, name: string): SshTaskSaveInput {
  return {
    id,
    defaultConnectionId: null,
    description: "",
    name,
    steps: [],
    workspaceId: "workspace-1",
  };
}
