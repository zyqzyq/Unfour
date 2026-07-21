import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
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
  Tabs,
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
import {
  closeTaskTab,
  createEmptyTaskEditorState,
  hydrateTaskTab,
  isTaskTabDirty,
  openNewTaskTab,
  openSavedTaskTab,
  persistTaskTab,
  removeTaskTabs,
  updateTaskTabDraft,
  updateTaskTabView,
  type SshTaskDetailView,
} from "../model/task-editor-tabs";

// eslint-disable-next-line max-lines-per-function -- coordinates task tabs, queries, mutations, sidebar content, and task-run dialogs in one page boundary
export function SshTasksPage({
  connections,
  onOpenConnections,
  onShellSidebarChange,
  workspaceId,
}: {
  connections: SshConnection[];
  onOpenConnections: () => void;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const [editorState, setEditorState] = useState(createEmptyTaskEditorState);
  const [closeTabId, setCloseTabId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<SshTask | null>(null);
  const [clearTaskId, setClearTaskId] = useState<string | null>(null);
  const [runDialogTask, setRunDialogTask] = useState<SshTaskDetail | null>(null);
  const [runConnectionId, setRunConnectionId] = useState("");
  const [runInputs, setRunInputs] = useState<Record<string, string>>({});
  const [activeRun, setActiveRun] = useState<SshTaskRun | null>(null);
  const [activeRunTask, setActiveRunTask] = useState<SshTaskDetail | null>(null);
  const [eventsByRun, setEventsByRun] = useState<Record<string, SshTaskRunEvent[]>>({});
  const nextNewTabIdRef = useRef(0);

  const tasksQuery = useQuery({
    queryKey: ["ssh-tasks", workspaceId],
    queryFn: () => listSshTasks(workspaceId),
  });
  const tasks = useMemo(() => tasksQuery.data ?? [], [tasksQuery.data]);
  const activeTab =
    editorState.tabs.find((tab) => tab.id === editorState.activeTabId) ?? null;
  const activeTaskId = activeTab?.taskId ?? null;
  const draft = activeTab?.draft ?? null;

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
    if (!detail || !activeTab || activeTab.taskId !== detail.task.id || activeTab.draft) {
      return;
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- React Query detail hydration seeds the matching editor tab without replacing other drafts
    setEditorState((current) => hydrateTaskTab(current, activeTab.id, toDraft(detail)));
  }, [activeTab, detailQuery.data]);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    registerSshTaskRunChannel((event) => {
      setEventsByRun((current) => ({
        ...current,
        [event.runId]: [...(current[event.runId] ?? []), event].slice(-5_000),
      }));
      if (event.kind === "run" && event.status && event.status !== "running") {
        queryClient.invalidateQueries({
          queryKey: ["ssh-task-runs", workspaceId, event.taskId],
        });
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
    mutationFn: ({ draft: input }: { draft: SshTaskSaveInput; tabId: string }) =>
      saveSshTask(input),
    onSuccess: (detail, { tabId }) => {
      const savedDraft = toDraft(detail);
      setEditorState((current) => persistTaskTab(current, tabId, savedDraft));
      queryClient.setQueryData(["ssh-task", workspaceId, detail.task.id], detail);
      queryClient.invalidateQueries({ queryKey: ["ssh-tasks", workspaceId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskSaveFailed" }),
  });
  const duplicateMutation = useMutation({
    mutationFn: (taskId: string) => duplicateSshTask(workspaceId, taskId),
    onSuccess: (detail) => {
      const savedDraft = toDraft(detail);
      setEditorState((current) => {
        const opened = openSavedTaskTab(current, detail.task.id);
        return hydrateTaskTab(opened, opened.activeTabId!, savedDraft);
      });
      queryClient.setQueryData(["ssh-task", workspaceId, detail.task.id], detail);
      queryClient.invalidateQueries({ queryKey: ["ssh-tasks", workspaceId] });
    },
    onError: (error) => handleError(error, { key: "feedback.ssh.taskDuplicateFailed" }),
  });
  const deleteMutation = useMutation({
    mutationFn: (taskId: string) => deleteSshTask(workspaceId, taskId),
    onSuccess: (_, taskId) => {
      setEditorState((current) => removeTaskTabs(current, taskId));
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
      queryClient.invalidateQueries({
        queryKey: ["ssh-task-runs", workspaceId, run.taskId],
      });
    },
  });
  const cancelMutation = useMutation({
    mutationFn: () => cancelSshTaskRun({ workspaceId, runId: activeRun!.id }),
    onError: (error) => handleError(error, { key: "feedback.ssh.taskCancelFailed" }),
  });
  const clearMutation = useMutation({
    mutationFn: (taskId: string) => clearSshTaskRuns({ workspaceId, taskId }),
    onSuccess: (_, taskId) => {
      setClearTaskId(null);
      queryClient.invalidateQueries({
        queryKey: ["ssh-task-runs", workspaceId, taskId],
      });
    },
    onError: (error) =>
      handleError(error, { key: "feedback.ssh.taskHistoryClearFailed" }),
  });
  const duplicateTask = duplicateMutation.mutate;
  const resetRunMutation = runMutation.reset;

  const newTask = useCallback(
    (template?: SshTaskSaveInput) => {
      const tabId = `new:${++nextNewTabIdRef.current}`;
      setEditorState((current) =>
        openNewTaskTab(
          current,
          tabId,
          template ?? {
            workspaceId,
            name: "",
            description: "",
            defaultConnectionId: null,
            steps: [],
          },
        ),
      );
    },
    [workspaceId],
  );

  const selectTask = useCallback((taskId: string) => {
    setEditorState((current) => openSavedTaskTab(current, taskId));
  }, []);

  const prepareRun = useCallback(
    async (taskId: string) => {
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
        resetRunMutation();
      } catch (error) {
        handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    },
    [handleError, queryClient, resetRunMutation, workspaceId],
  );

  const handleListRun = useCallback(
    (task: SshTask) => void prepareRun(task.id),
    [prepareRun],
  );
  const handleDuplicate = useCallback(
    (task: SshTask) => duplicateTask(task.id),
    [duplicateTask],
  );
  const handleExample = useCallback(
    () => newTask(dockerImageExportTemplate(workspaceId)),
    [newTask, workspaceId],
  );

  const shellSidebar = useMemo(
    () => (
      <TaskList
        loading={tasksQuery.isLoading}
        onDelete={setDeleteTarget}
        onDuplicate={handleDuplicate}
        onExample={handleExample}
        onNew={newTask}
        onOpenConnections={onOpenConnections}
        onRun={handleListRun}
        onSelect={selectTask}
        selectedTaskId={activeTaskId}
        tasks={tasks}
      />
    ),
    [
      activeTaskId,
      handleDuplicate,
      handleExample,
      handleListRun,
      newTask,
      onOpenConnections,
      selectTask,
      tasks,
      tasksQuery.isLoading,
    ],
  );

  useEffect(() => {
    if (!onShellSidebarChange) return;
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [onShellSidebarChange, shellSidebar]);

  function requestCloseTab(tabId: string) {
    const tab = editorState.tabs.find((item) => item.id === tabId);
    if (tab && isTaskTabDirty(tab)) {
      setCloseTabId(tabId);
      return;
    }
    setEditorState((current) => closeTaskTab(current, tabId));
  }

  function updateDraft(nextDraft: SshTaskSaveInput) {
    if (!activeTab) return;
    setEditorState((current) => updateTaskTabDraft(current, activeTab.id, nextDraft));
  }

  function updateDetailView(view: SshTaskDetailView) {
    if (!activeTab) return;
    setEditorState((current) => updateTaskTabView(current, activeTab.id, view));
  }

  const closeTarget = editorState.tabs.find((tab) => tab.id === closeTabId) ?? null;
  const showEditor = activeTab?.view !== "history";
  const taskTabs = editorState.tabs.map((tab) => {
    const task = tab.taskId ? tasks.find((item) => item.id === tab.taskId) : null;
    const title = tab.draft?.name.trim() || task?.name || t("ssh.tasks.editor.untitled");
    const dirty = isTaskTabDirty(tab);
    return {
      id: tab.id,
      title,
      draggable: false,
      meta: dirty ? (
        <span
          aria-label={t("ssh.tasks.tabs.unsaved")}
          className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--u-color-primary)]"
          title={t("ssh.tasks.tabs.unsaved")}
        />
      ) : undefined,
    };
  });

  return (
    <div className="flex min-h-0 flex-1 bg-[var(--u-color-surface)]">
      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        <Tabs
          activeId={activeTab?.id ?? ""}
          endControl={
            activeTab ? (
              <SegmentedControl
                className="w-[196px]"
                onChange={updateDetailView}
                options={[
                  { label: t("ssh.tasks.tabs.editor"), value: "editor" },
                  { label: t("ssh.tasks.tabs.history"), value: "history" },
                ]}
                value={activeTab.view}
              />
            ) : undefined
          }
          onClose={requestCloseTab}
          onSelect={(tabId) =>
            setEditorState((current) => ({ ...current, activeTabId: tabId }))
          }
          tabs={taskTabs}
        />
        {detailQuery.isError && !draft ? (
          <ErrorState className="min-h-0 flex-1">{String(detailQuery.error)}</ErrorState>
        ) : draft ? (
          showEditor ? (
            <TaskEditor
              connections={connections}
              draft={draft}
              onChange={updateDraft}
              onRun={() => activeTaskId && void prepareRun(activeTaskId)}
              onSave={() =>
                activeTab && saveMutation.mutate({ draft, tabId: activeTab.id })
              }
              saving={
                saveMutation.isPending && saveMutation.variables?.tabId === activeTab?.id
              }
            />
          ) : activeTaskId ? (
            <TaskHistory
              clearing={
                clearMutation.isPending && clearMutation.variables === activeTaskId
              }
              loading={runsQuery.isLoading}
              onClear={() => setClearTaskId(activeTaskId)}
              runs={runsQuery.data ?? []}
            />
          ) : (
            <div className="flex flex-1 items-center justify-center text-[12px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.history.saveFirst")}
            </div>
          )
        ) : (
          <div className="flex flex-1 items-center justify-center text-[12px] text-[var(--u-color-text-muted)]">
            {activeTab
              ? t("ssh.tasks.list.loading")
              : tasks.length
                ? t("ssh.tasks.list.selectTask")
                : t("ssh.tasks.list.emptyDescription")}
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
        onInputChange={(name, value) =>
          setRunInputs((current) => ({ ...current, [name]: value }))
        }
        onOpenChange={(open) => !open && setRunDialogTask(null)}
        onRun={() => runMutation.mutate()}
        open={runDialogTask !== null}
        pending={runMutation.isPending}
        task={runDialogTask}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.tasks.tabs.discard")}
        description={
          closeTarget
            ? t("ssh.tasks.tabs.discardDescription", {
                name:
                  closeTarget.draft?.name.trim() || t("ssh.tasks.editor.untitled"),
              })
            : ""
        }
        onConfirm={() => {
          if (closeTabId) {
            setEditorState((current) => closeTaskTab(current, closeTabId));
          }
          setCloseTabId(null);
        }}
        onOpenChange={(open) => !open && setCloseTabId(null)}
        open={closeTarget !== null}
        title={t("ssh.tasks.tabs.discardTitle")}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.tasks.actions.delete")}
        description={
          deleteTarget ? t("ssh.tasks.confirmDelete", { name: deleteTarget.name }) : ""
        }
        onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        open={deleteTarget !== null}
        pending={deleteMutation.isPending}
        title={t("ssh.tasks.confirmDeleteTitle")}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.tasks.history.clear")}
        description={t("ssh.tasks.history.clearDescription")}
        onConfirm={() => clearTaskId && clearMutation.mutate(clearTaskId)}
        onOpenChange={(open) => !open && setClearTaskId(null)}
        open={clearTaskId !== null}
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
