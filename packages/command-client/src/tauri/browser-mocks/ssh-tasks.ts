import type {
  SshTaskCleanupInput,
  SshTaskDetail,
  SshTaskRun,
  SshTaskRunInput,
  SshTaskSaveInput,
  SshTasksReorderInput,
} from "../../types";
import { mockStore } from "./state";
import { UNHANDLED, type MockResult } from "./types";

type TaskMockHandler = <T>(
  command: string,
  args?: Record<string, unknown>,
) => MockResult<T>;

const TASK_MOCK_HANDLERS: TaskMockHandler[] = [
  handleTaskListMock,
  handleTaskReorderMock,
  handleTaskGetMock,
  handleTaskSaveMock,
  handleTaskDuplicateMock,
  handleTaskDeleteMock,
  handleTaskStartRunMock,
  handleTaskCancelRunMock,
  handleTaskRunsListMock,
  handleTaskRunLogReadMock,
  handleTaskRunsClearMock,
];

export function handleSshTaskMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  for (const handler of TASK_MOCK_HANDLERS) {
    const result = handler<T>(command, args);
    if (result !== UNHANDLED) return result;
  }
  return UNHANDLED;
}

function handleTaskListMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_tasks_list") return UNHANDLED;
  const workspaceId = String(args?.workspaceId ?? "");
  return mockStore.sshTasks
    .filter(
      (detail) =>
        detail.task.workspaceId === workspaceId && detail.task.deletedAt === null,
    )
    .sort((left, right) => left.task.sortOrder - right.task.sortOrder)
    .map((detail) => detail.task) as T;
}

function handleTaskReorderMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_tasks_reorder") return UNHANDLED;
  const input = args?.input as SshTasksReorderInput;
  const active = mockStore.sshTasks.filter(
    (detail) =>
      detail.task.workspaceId === input.workspaceId && detail.task.deletedAt === null,
  );
  const unique = new Set(input.taskIds);
  if (
    unique.size !== input.taskIds.length ||
    active.length !== input.taskIds.length ||
    active.some((detail) => !unique.has(detail.task.id))
  ) {
    throw new Error(
      "SSH task reorder must contain every active task in the workspace exactly once",
    );
  }
  input.taskIds.forEach((taskId, sortOrder) => {
    const detail = active.find((item) => item.task.id === taskId)!;
    if (detail.task.sortOrder !== sortOrder) {
      detail.task.sortOrder = sortOrder;
    }
  });
  return active
    .slice()
    .sort((left, right) => left.task.sortOrder - right.task.sortOrder)
    .map((detail) => structuredClone(detail.task)) as T;
}

function handleTaskGetMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_get") return UNHANDLED;
  const detail = activeTask(
    String(args?.workspaceId ?? ""),
    String(args?.taskId ?? ""),
  );
  return structuredClone(detail) as T;
}

function handleTaskSaveMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_save") return UNHANDLED;
  return saveTask(args?.input as SshTaskSaveInput) as T;
}

function handleTaskDuplicateMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_duplicate") return UNHANDLED;
  const workspaceId = String(args?.workspaceId ?? "");
  const detail = activeTask(workspaceId, String(args?.taskId ?? ""));
  return saveTask({
    workspaceId,
    name: `${detail.task.name} Copy`,
    description: detail.task.description,
    defaultConnectionId: detail.localBinding?.defaultConnectionId ?? null,
    steps: detail.steps.map((step) => ({
      name: step.name,
      stepType: step.stepType,
      position: step.position,
      enabled: step.enabled,
      configVersion: step.configVersion,
      configJson: structuredClone(step.configJson),
    })),
  }) as T;
}

function handleTaskDeleteMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_delete") return UNHANDLED;
  const detail = activeTask(
    String(args?.workspaceId ?? ""),
    String(args?.taskId ?? ""),
  );
  const now = new Date().toISOString();
  detail.task.deletedAt = now;
  detail.task.updatedAt = now;
  detail.steps = [];
  detail.localBinding = null;
  return undefined as T;
}

function handleTaskStartRunMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_run") return UNHANDLED;
  const input = args?.input as SshTaskRunInput;
  const detail = activeTask(input.workspaceId, input.taskId);
  const connectionId = preferredConnectionId(input, detail);
  const now = new Date().toISOString();
  detail.localBinding = {
    taskId: input.taskId,
    workspaceId: input.workspaceId,
    defaultConnectionId: detail.localBinding?.defaultConnectionId ?? null,
    lastUsedConnectionId: connectionId,
    createdAt: detail.localBinding?.createdAt ?? now,
    updatedAt: now,
  };
  const run: SshTaskRun = {
    id: crypto.randomUUID(),
    workspaceId: input.workspaceId,
    taskId: input.taskId,
    connectionId,
    status: "success",
    startedAt: now,
    finishedAt: now,
    errorMessage: null,
    logPath: `~/.unfour/logs/tasks/mock-${Date.now()}.log`,
  };
  mockStore.sshTaskRuns.unshift(run);
  return run as T;
}

function handleTaskCancelRunMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_run_cancel") return UNHANDLED;
  const runId = String((args?.input as { runId?: string })?.runId ?? "");
  const run = mockStore.sshTaskRuns.find((item) => item.id === runId);
  if (!run) throw new Error("SSH task run not found");
  run.status = "cancelled";
  run.finishedAt = new Date().toISOString();
  return run as T;
}

function handleTaskRunsListMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_runs_list") return UNHANDLED;
  const workspaceId = String(args?.workspaceId ?? "");
  const taskId = String(args?.taskId ?? "");
  return mockStore.sshTaskRuns.filter(
    (run) => run.workspaceId === workspaceId && run.taskId === taskId,
  ) as T;
}

function handleTaskRunLogReadMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_run_log_read") return UNHANDLED;
  const workspaceId = String(args?.workspaceId ?? "");
  const runId = String(args?.runId ?? "");
  const run = mockStore.sshTaskRuns.find(
    (item) => item.workspaceId === workspaceId && item.id === runId,
  );
  if (!run) throw new Error("SSH task run not found");
  const started = run.startedAt;
  return [
    `[${started}] run running`,
    `[${started}] step 'Mock command' running`,
    `[${started}] command $ echo hello from mock task run`,
    `[${started}] stdout hello from mock task run`,
    `[${started}] step 'Mock command' success`,
    `[${run.finishedAt ?? started}] run ${run.status}`,
  ].join("\n") as T;
}

function handleTaskRunsClearMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command !== "ssh_task_runs_clear") return UNHANDLED;
  const input = args?.input as SshTaskCleanupInput;
  const before = mockStore.sshTaskRuns.length;
  mockStore.sshTaskRuns = mockStore.sshTaskRuns.filter(
    (run) =>
      run.workspaceId !== input.workspaceId ||
      (input.taskId !== null && run.taskId !== input.taskId),
  );
  const deletedRuns = before - mockStore.sshTaskRuns.length;
  return { deletedRuns, deletedLogs: deletedRuns } as T;
}

function detectedTaskInputs(steps: SshTaskDetail["steps"]): string[] {
  const fields: Record<string, string[]> = {
    command: ["command", "workingDirectory"],
    upload: ["localPath", "remotePath"],
    download: ["remotePath", "localPath"],
  };
  const names: string[] = [];
  for (const step of steps ?? []) {
    if (!step.enabled) continue;
    const config = step.configJson as unknown as Record<string, unknown>;
    for (const field of fields[step.stepType] ?? []) {
      const value = config?.[field];
      if (typeof value !== "string") continue;
      for (const match of value.matchAll(/\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}/g)) {
        if (!names.includes(match[1])) names.push(match[1]);
      }
    }
  }
  return names;
}

function activeTask(workspaceId: string, taskId: string): SshTaskDetail {
  const detail = mockStore.sshTasks.find(
    (item) =>
      item.task.workspaceId === workspaceId &&
      item.task.id === taskId &&
      item.task.deletedAt === null,
  );
  if (!detail) throw new Error("SSH task not found");
  return { ...detail, detectedInputs: detail.detectedInputs ?? detectedTaskInputs(detail.steps) };
}

function preferredConnectionId(
  input: SshTaskRunInput,
  detail: SshTaskDetail,
): string {
  const connectionId =
    input.connectionId ??
    detail.localBinding?.defaultConnectionId ??
    detail.localBinding?.lastUsedConnectionId;
  if (!connectionId) throw new Error("SSH task run requires a connection");
  return connectionId;
}

function saveTask(input: SshTaskSaveInput): SshTaskDetail {
  const now = new Date().toISOString();
  const existing = input.id
    ? mockStore.sshTasks.find((detail) => detail.task.id === input.id)
    : undefined;
  const taskId = input.id ?? crypto.randomUUID();
  const localBinding =
    input.defaultConnectionId !== null || existing?.localBinding
      ? {
          taskId,
          workspaceId: input.workspaceId,
          defaultConnectionId: input.defaultConnectionId,
          lastUsedConnectionId: existing?.localBinding?.lastUsedConnectionId ?? null,
          createdAt: existing?.localBinding?.createdAt ?? now,
          updatedAt: now,
        }
      : null;
  const detail: SshTaskDetail = {
    task: {
      id: taskId,
      workspaceId: input.workspaceId,
      name: input.name.trim(),
      description: input.description.trim(),
      sortOrder: taskSortOrder(existing, input.workspaceId),
      createdAt: existing?.task.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
    },
    steps: input.steps
      .slice()
      .sort((left, right) => left.position - right.position)
      .map((step, position) => ({
        ...step,
        id: step.id ?? crypto.randomUUID(),
        workspaceId: input.workspaceId,
        taskId,
        position,
        configVersion:
          existing?.steps.find((item) => item.id === step.id)?.configVersion ??
          step.configVersion ??
          1,
        createdAt:
          existing?.steps.find((item) => item.id === step.id)?.createdAt ?? now,
        updatedAt: now,
        deletedAt: null,
      })),
    localBinding,
    detectedInputs: detectedTaskInputs(input.steps as SshTaskDetail["steps"]),
  };
  mockStore.sshTasks = [
    detail,
    ...mockStore.sshTasks.filter((item) => item.task.id !== taskId),
  ];
  return structuredClone(detail);
}

function taskSortOrder(existing: SshTaskDetail | undefined, workspaceId: string) {
  if (existing) return existing.task.sortOrder;
  return (
    Math.max(
      -1,
      ...mockStore.sshTasks
        .filter(
          (detail) =>
            detail.task.workspaceId === workspaceId && detail.task.deletedAt === null,
        )
        .map((detail) => detail.task.sortOrder),
    ) + 1
  );
}
