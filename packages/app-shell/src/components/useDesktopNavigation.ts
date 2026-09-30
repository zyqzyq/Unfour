import { useCallback, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { setActiveWorkspace as setActiveWorkspaceCommand, type WorkspaceState, type WorkspaceTab } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useFeedbackErrorHandler } from "@unfour/ui";
import type { DesktopWorkspaceController } from "./useDesktopWorkspace";
import type { useFeatureModulePreload } from "./useFeatureModulePreload";

type VariableManagerLeave =
  | { kind: "activate-workspace"; workspaceId: string }
  | { kind: "select-module"; tabId: string };

function useWorkspaceActivation(onActivated: () => void) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const { activeWorkspaceId, setActiveWorkspace } = useWorkspaceStore();
  return useMutation({
    mutationFn: setActiveWorkspaceCommand,
    onMutate: async (workspaceId) => {
      await queryClient.cancelQueries({ queryKey: ["workspaces"] });
      const previousState = queryClient.getQueryData<WorkspaceState>(["workspaces"]);
      const previousWorkspaceId = activeWorkspaceId;
      if (previousState) {
        queryClient.setQueryData<WorkspaceState>(["workspaces"], { ...previousState, activeWorkspaceId: workspaceId });
      }
      setActiveWorkspace(workspaceId);
      return { previousState, previousWorkspaceId };
    },
    onSuccess: (state) => {
      onActivated();
      queryClient.setQueryData<WorkspaceState>(["workspaces"], state);
      setActiveWorkspace(state.activeWorkspaceId);
    },
    onError: (error, _workspaceId, context) => {
      if (context?.previousState) queryClient.setQueryData(["workspaces"], context.previousState);
      const rollbackWorkspaceId = context?.previousWorkspaceId ?? context?.previousState?.activeWorkspaceId;
      if (rollbackWorkspaceId) setActiveWorkspace(rollbackWorkspaceId);
      handleError(error, { key: "feedback.workspace.activateFailed" });
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
  });
}

/** Navigation and the unsaved workspace-variable draft's leave guard. */
export function useDesktopNavigation({ activeWorkspace, activeEnvironmentId, tabs, setActiveTab, preloadFeature }: {
  activeWorkspace: DesktopWorkspaceController["activeWorkspace"];
  activeEnvironmentId: string | null;
  tabs: readonly WorkspaceTab[];
  setActiveTab: (tabId: string) => void;
  preloadFeature: ReturnType<typeof useFeatureModulePreload>;
}) {
  const [variableManagerRequest, setVariableManagerRequest] = useState<{
    environmentId: string | null; workspaceId: string;
  } | null>(null);
  const [variableManagerDirty, setVariableManagerDirty] = useState(false);
  const [pendingVariableManagerLeave, setPendingVariableManagerLeave] = useState<VariableManagerLeave | null>(null);
  const closeVariableManager = useCallback(() => {
    setVariableManagerRequest(null);
    setVariableManagerDirty(false);
    setPendingVariableManagerLeave(null);
  }, []);
  const activateWorkspaceMutation = useWorkspaceActivation(closeVariableManager);
  const variableManagerOpen = Boolean(activeWorkspace) && variableManagerRequest?.workspaceId === activeWorkspace?.id;
  const handleManageVariables = useCallback(() => {
    if (!activeWorkspace || variableManagerOpen) return;
    setVariableManagerRequest({ environmentId: activeEnvironmentId, workspaceId: activeWorkspace.id });
  }, [activeEnvironmentId, activeWorkspace, variableManagerOpen]);
  const applyVariableManagerLeave = useCallback((leave: VariableManagerLeave) => {
    closeVariableManager();
    if (leave.kind === "select-module") {
      setActiveTab(leave.tabId);
      return;
    }
    if (leave.kind === "activate-workspace") activateWorkspaceMutation.mutate(leave.workspaceId);
  }, [activateWorkspaceMutation, closeVariableManager, setActiveTab]);
  const requestLeaveVariableManager = useCallback((leave: VariableManagerLeave) => {
    if (!variableManagerOpen) {
      if (leave.kind === "select-module") {
        setActiveTab(leave.tabId);
        return;
      }
      activateWorkspaceMutation.mutate(leave.workspaceId);
      return;
    }
    if (variableManagerDirty) {
      setPendingVariableManagerLeave(leave);
      return;
    }
    applyVariableManagerLeave(leave);
  }, [activateWorkspaceMutation, applyVariableManagerLeave, setActiveTab, variableManagerDirty, variableManagerOpen]);
  const handleSelectModule = useCallback((tabId: string) => {
    const kind = tabs.find((tab) => tab.id === tabId)?.kind;
    if (kind) void preloadFeature(kind).catch(() => undefined);
    requestLeaveVariableManager({ kind: "select-module", tabId });
  }, [preloadFeature, requestLeaveVariableManager, tabs]);
  const handleActivateWorkspace = useCallback((workspaceId: string) => {
    if (workspaceId === activeWorkspace?.id || activateWorkspaceMutation.isPending) return;
    requestLeaveVariableManager({ kind: "activate-workspace", workspaceId });
  }, [activateWorkspaceMutation.isPending, activeWorkspace?.id, requestLeaveVariableManager]);
  const confirmVariableManagerLeave = () => {
    if (pendingVariableManagerLeave) applyVariableManagerLeave(pendingVariableManagerLeave);
  };
  const cancelVariableManagerLeave = () => setPendingVariableManagerLeave(null);
  return {
    variableManagerRequest, variableManagerOpen, pendingVariableManagerLeave, setVariableManagerDirty,
    closeVariableManager, confirmVariableManagerLeave, cancelVariableManagerLeave,
    handleManageVariables, handleSelectModule, handleActivateWorkspace,
  };
}

export type DesktopNavigationController = ReturnType<typeof useDesktopNavigation>;
