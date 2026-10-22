import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  cancelSshTaskRun,
  clearSshTaskRuns,
  getSshTask,
  type SshTaskDetail,
  type SshTaskRun,
} from "@unfour/command-client";
import { useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import { useSshTaskRunOutput } from "./useSshTaskRunOutput";

import { useTaskRunDialogState } from "./useTaskRunDialogState";

export function useSshTaskRunSession({
  active,
  handleError,
  workspaceId,
}: {
  active: boolean;
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();
  const feedback = useFeedback();
  const { t } = useI18n();
  const noteActivityWarning = (warnings?: string[]) => {
    if (warnings?.includes("ACTIVITY_RECORD_FAILED")) {
      feedback.info(t("feedback.activityRecordFailed"));
    }
  };
  const showCompletionWarning = (run: SshTaskRun) => noteActivityWarning(run.warnings);
  const [activeRun, setActiveRunState] = useState<SshTaskRun | null>(null);
  const [activeRunTask, setActiveRunTask] = useState<SshTaskDetail | null>(null);
  const workspaceScope = useMemo(() => ({ workspaceId }), [workspaceId]);
  const mountedScopeRef = useRef<typeof workspaceScope | null>(null);
  const sessionGenerationRef = useRef(0);
  const historySelectionVersionRef = useRef(0);
  const historySelectionTaskIdRef = useRef<string | null>(null);

  useEffect(() => {
    mountedScopeRef.current = workspaceScope;
    sessionGenerationRef.current += 1;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- A run selection belongs to the current workspace.
    setActiveRunState(null);
    setActiveRunTask(null);
    return () => {
      mountedScopeRef.current = null;
      sessionGenerationRef.current += 1;
    };
  }, [workspaceScope]);

  const setActiveRun = useCallback<typeof setActiveRunState>((run) => {
    historySelectionVersionRef.current += 1;
    historySelectionTaskIdRef.current = null;
    setActiveRunState(run);
  }, []);

  const dialog = useTaskRunDialogState({ workspaceId, handleError, onStarted: (run, detail) => {
    setActiveRun(run);
    setActiveRunTask(detail);
    showCompletionWarning(run);
  } });

  const {
    eventsByRun, historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskOutput,
  } = useSshTaskRunOutput({ active, activeRunId: activeRun?.id ?? null, handleError, workspaceId });

  const cancelMutation = useMutation({
    mutationFn: () => cancelSshTaskRun({ workspaceId, runId: activeRun!.id }),
    onSuccess: showCompletionWarning,
    onError: (error) => handleError(error, { key: "feedback.ssh.taskCancelFailed" }),
  });
  const clearMutation = useMutation({
    mutationFn: async (taskId: string) => {
      const generation = sessionGenerationRef.current;
      const isCurrent = () => mountedScopeRef.current === workspaceScope && generation === sessionGenerationRef.current;
      try {
        const result = await clearSshTaskRuns({ workspaceId, taskId });
        if (isCurrent()) {
          if (historySelectionTaskIdRef.current === taskId) historySelectionVersionRef.current += 1;
          clearTaskOutput(taskId);
          noteActivityWarning(result?.warnings);
        }
        // The backend mutation still changed this workspace if the view moved.
        queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, taskId] });
        return result;
      } catch (error) {
        if (isCurrent()) handleError(error, { key: "feedback.ssh.taskHistoryClearFailed" });
        throw error;
      }
    },
  });
  const openHistoryRun = useCallback(
    async (run: SshTaskRun) => {
      if (mountedScopeRef.current !== workspaceScope) return;
      const generation = sessionGenerationRef.current;
      const selectionVersion = ++historySelectionVersionRef.current;
      historySelectionTaskIdRef.current = run.taskId;
      const isCurrent = () => mountedScopeRef.current === workspaceScope && generation === sessionGenerationRef.current &&
        selectionVersion === historySelectionVersionRef.current;
      try {
        const detail = await queryClient.fetchQuery({
          queryKey: ["ssh-task", workspaceId, run.taskId],
          queryFn: () => getSshTask(workspaceId, run.taskId),
        });
        if (!isCurrent()) return;
        setActiveRunState(run);
        setActiveRunTask(detail);

        await loadHistoryLog(run.id, run.taskId);
      } catch (error) {
        if (isCurrent()) handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    },
    [handleError, loadHistoryLog, queryClient, workspaceScope, workspaceId],
  );

  return {
    ...dialog,
    activeRun, activeRunTask, cancelMutation, clearMutation,
    eventsByRun, historyLogByRun, historyLogLoading, openHistoryRun,
    setActiveRun, setActiveRunTask,
  };
}
