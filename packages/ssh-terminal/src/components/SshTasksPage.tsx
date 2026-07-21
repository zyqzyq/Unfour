import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  cancelSshTaskRun,
  clearSshTaskRuns,
  deleteSshTask,
  duplicateSshTask,
  getSshTask,
  listSshTaskRuns,
  listSshTasks,
  registerSshTaskRunChannel,
  runSshTask,
  saveSshTask,
  type SshConnection,
  type SshTask,
  type SshTaskDetail,
  type SshTaskRun,
  type SshTaskRunEvent,
  type SshTaskSaveInput,
} from "@unfour/command-client";
import {
  ConfirmDialog,
  ErrorState,
  SegmentedControl,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import { TaskEditor } from "./TaskEditor";
import { TaskHistory } from "./TaskHistory";
import { TaskList } from "./TaskList";
import { TaskRunDialog } from "./TaskRunDialog";
import { TaskRunPanel } from "./TaskRunPanel";
import {
  detectTaskInputs,
  dockerImageExportTemplate,
  preferredTaskConnectionId,
} from "../model/task-template";

type DetailView = "editor" | "history";

export function SshTasksPage({
  connections,
  workspaceId,
}: {
  connections: SshConnection[];
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<SshTaskSaveInput | null>(null);
  const [detailView, setDetailView] = useState<DetailView>("editor");
  const [deleteTarget, setDeleteTarget] = useState<SshTask | null>(null);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [runDialogTask, setRunDialogTask] = useState<SshTaskDetail | null>(null);
  const [runConnectionId, setRunConnectionId] = useState("");
  const [runInputs, setRunInputs] = useState<Record<string, string>>({});
  const [activeRun, setActiveRun] = useState<SshTaskRun | null>(null);
  const [activeRunTask, setActiveRunTask] = useState<SshTaskDetail | null>(null);
  const [eventsByRun, setEventsByRun] = useState<Record<string, SshTaskRunEvent[]>>({});
  const loadedTaskIdRef = useRef<string | null>(null);

  const tasksQuery = useQuery({
    queryKey: ["ssh-tasks", workspaceId],
    queryFn: () => listSshTasks(workspaceId),
  });
  const tasks = useMemo(() => tasksQuery.data ?? [], [tasksQuery.data]);
  const activeTaskId = creating ? null : (selectedTaskId ?? tasks[0]?.id ?? null);
  const detailQuery = useQuery({
    queryKey: ["ssh-task", workspaceId, activeTaskId],
    queryFn: () => getSshTask(workspaceId, activeTaskId!),
    enabled: Boolean(activeTaskId),
  });
  const runsQuery = useQuery({
    queryKey: ["ssh-task-runs", workspaceId, activeTaskId],
    queryFn: () => listSshTaskRuns(workspaceId, activeTaskId!),
    enabled: Boolean(activeTaskId),
  });

  useEffect(() => {
    const detail = detailQuery.data;
    if (!detail || loadedTaskIdRef.current === detail.task.id) return;
    loadedTaskIdRef.current = detail.task.id;
    setDraft(toDraft(detail));
  }, [detailQuery.data]);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    registerSshTaskRunChannel((event) => {
      setEventsByRun((current) => ({
        ...current,
        [event.runId]: [...(current[event.runId] ?? []), event].slice(-5_000),
      }));
      if (event.kind === "run" && event.status && event.status !== "running") {
        queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, event.taskId] });
      }
    }).then((cleanup) => {
      if (disposed) cleanup();
      else dispose = cleanup;
    });
    return () => {
      disposed = true;
      dispose?.();
    };
  }, [queryClient, workspaceId]);

  const saveMutation = useMutation({
    mutationFn: saveSshTask,
    onSuccess: (detail) => {
      loadedTaskIdRef.current = detail.task.id;
      setCreating(false);
      setSelectedTaskId(detail.task.id);
      setDraft(toDraft(detail));
      queryClient.setQueryData(["ssh-task", workspaceId, detail.task.id], detail);
      queryClient.invalidateQueries({ queryKey: ["ssh-tasks", workspaceId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskSaveFailed" }),
  });
  const duplicateMutation = useMutation({
    mutationFn: (taskId: string) => duplicateSshTask(workspaceId, taskId),
    onSuccess: (detail) => {
      loadedTaskIdRef.current = detail.task.id;
      setSelectedTaskId(detail.task.id);
      setCreating(false);
      setDraft(toDraft(detail));
      queryClient.setQueryData(["ssh-task", workspaceId, detail.task.id], detail);
      queryClient.invalidateQueries({ queryKey: ["ssh-tasks", workspaceId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskDuplicateFailed" }),
  });
  const deleteMutation = useMutation({
    mutationFn: (taskId: string) => deleteSshTask(workspaceId, taskId),
    onSuccess: (_, taskId) => {
      if (activeTaskId === taskId) {
        setSelectedTaskId(null);
        setDraft(null);
        loadedTaskIdRef.current = null;
      }
      setDeleteTarget(null);
      queryClient.invalidateQueries({ queryKey: ["ssh-tasks", workspaceId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskDeleteFailed" }),
  });
  const runMutation = useMutation({
    mutationFn: () =>
      runSshTask({
        workspaceId,
        taskId: runDialogTask!.task.id,
        connectionId: runConnectionId || null,
        inputs: runInputs,
      }),
    onSuccess: (run) => {
      setActiveRun(run);
      setActiveRunTask(runDialogTask);
      setRunDialogTask(null);
      queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, run.taskId] });
    },
  });
  const cancelMutation = useMutation({
    mutationFn: () => cancelSshTaskRun({ workspaceId, runId: activeRun!.id }),
    onError: (error) => handleError(error, { key: "feedback.ssh.taskCancelFailed" }),
  });
  const clearMutation = useMutation({
    mutationFn: () => clearSshTaskRuns({ workspaceId, taskId: activeTaskId }),
    onSuccess: () => {
      setClearConfirmOpen(false);
      queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, activeTaskId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskHistoryClearFailed" }),
  });

  function newTask(template?: SshTaskSaveInput) {
    setCreating(true);
    setSelectedTaskId(null);
    loadedTaskIdRef.current = null;
    setDraft(
      template ?? {
        workspaceId,
        name: "",
        description: "",
        defaultConnectionId: null,
        steps: [],
      },
    );
    setDetailView("editor");
  }

  function selectTask(taskId: string) {
    setCreating(false);
    setSelectedTaskId(taskId);
    setDraft(null);
    loadedTaskIdRef.current = null;
  }

  async function openRun(task: SshTask | null = null) {
    const taskId = task?.id ?? activeTaskId;
    if (!taskId) return;
    try {
      const detail = await queryClient.fetchQuery({
        queryKey: ["ssh-task", workspaceId, taskId],
        queryFn: () => getSshTask(workspaceId, taskId),
      });
      setRunDialogTask(detail);
      setRunConnectionId(preferredTaskConnectionId(detail.localBinding));
      setRunInputs(
        Object.fromEntries(detectTaskInputs(detail.steps, true).map((name) => [name, ""])),
      );
      runMutation.reset();
    } catch (error) {
      handleError(error, { key: "feedback.ssh.taskLoadFailed" });
    }
  }

  const showEditor = detailView === "editor";
  return (
    <div className="flex min-h-0 flex-1 bg-[var(--u-color-surface)]">
      <TaskList
        loading={tasksQuery.isLoading}
        onDelete={setDeleteTarget}
        onDuplicate={(task) => duplicateMutation.mutate(task.id)}
        onExample={() => newTask(dockerImageExportTemplate(workspaceId))}
        onNew={() => newTask()}
        onRun={(task) => void openRun(task)}
        onSelect={selectTask}
        selectedTaskId={activeTaskId}
        tasks={tasks}
      />
      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
          <SegmentedControl
            className="w-[220px]"
            onChange={setDetailView}
            options={[
              { label: t("ssh.tasks.tabs.editor"), value: "editor" },
              { label: t("ssh.tasks.tabs.history"), value: "history" },
            ]}
            value={detailView}
          />
        </div>
        {detailQuery.isError ? (
          <ErrorState className="min-h-0 flex-1">{String(detailQuery.error)}</ErrorState>
        ) : draft ? (
          showEditor ? (
            <TaskEditor
              connections={connections}
              draft={draft}
              onChange={setDraft}
              onRun={() => void openRun()}
              onSave={() => saveMutation.mutate(draft)}
              saving={saveMutation.isPending}
            />
          ) : activeTaskId ? (
            <TaskHistory
              clearing={clearMutation.isPending}
              loading={runsQuery.isLoading}
              onClear={() => setClearConfirmOpen(true)}
              runs={runsQuery.data ?? []}
            />
          ) : (
            <div className="flex flex-1 items-center justify-center text-[12px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.history.saveFirst")}
            </div>
          )
        ) : (
          <div className="flex flex-1 items-center justify-center text-[12px] text-[var(--u-color-text-muted)]">
            {tasks.length ? t("ssh.tasks.list.selectTask") : t("ssh.tasks.list.emptyDescription")}
          </div>
        )}
        {activeRun && activeRunTask && (
          <TaskRunPanel
            cancelling={cancelMutation.isPending}
            events={eventsByRun[activeRun.id] ?? []}
            onCancel={() => cancelMutation.mutate()}
            onClose={() => {
              setActiveRun(null);
              setActiveRunTask(null);
            }}
            run={activeRun}
            task={activeRunTask}
          />
        )}
      </main>

      <TaskRunDialog
        connectionId={runConnectionId}
        connections={connections}
        error={runMutation.error}
        inputValues={runInputs}
        onConnectionChange={setRunConnectionId}
        onInputChange={(name, value) => setRunInputs((current) => ({ ...current, [name]: value }))}
        onOpenChange={(open) => !open && setRunDialogTask(null)}
        onRun={() => runMutation.mutate()}
        open={runDialogTask !== null}
        pending={runMutation.isPending}
        task={runDialogTask}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.tasks.actions.delete")}
        description={deleteTarget ? t("ssh.tasks.confirmDelete", { name: deleteTarget.name }) : ""}
        onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        open={deleteTarget !== null}
        pending={deleteMutation.isPending}
        title={t("ssh.tasks.confirmDeleteTitle")}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.tasks.history.clear")}
        description={t("ssh.tasks.history.clearDescription")}
        onConfirm={() => clearMutation.mutate()}
        onOpenChange={setClearConfirmOpen}
        open={clearConfirmOpen}
        pending={clearMutation.isPending}
        title={t("ssh.tasks.history.clearTitle")}
      />
    </div>
  );
}

function toDraft(detail: SshTaskDetail): SshTaskSaveInput {
  return {
    id: detail.task.id,
    workspaceId: detail.task.workspaceId,
    name: detail.task.name,
    description: detail.task.description,
    defaultConnectionId: detail.localBinding?.defaultConnectionId ?? null,
    steps: detail.steps.map((step) => ({
      id: step.id,
      name: step.name,
      stepType: step.stepType,
      position: step.position,
      enabled: step.enabled,
      configVersion: step.configVersion,
      configJson: { ...step.configJson },
    })),
  };
}
