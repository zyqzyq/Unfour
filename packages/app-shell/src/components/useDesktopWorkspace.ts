import { useCallback } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getSystemHealth,
  getWorkspaceLayout,
  getWorkspaceState,
  listDatabaseConnections,
  listWorkspaceEnvironments,
  setActiveWorkspaceEnvironment,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useFeedbackErrorHandler } from "@unfour/ui";
import { useLayoutPersistence } from "./useLayoutPersistence";
import { useWorkspaceInit } from "./useWorkspaceInit";

/** Shell workspace queries, layout restoration and environment selection. */
export function useDesktopWorkspace() {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const { activeWorkspaceId } = useWorkspaceStore();
  const healthQuery = useQuery({ queryKey: ["system-health"], queryFn: getSystemHealth });
  const workspaceQuery = useQuery({ queryKey: ["workspaces"], queryFn: getWorkspaceState });
  const activeWorkspace =
    workspaceQuery.data?.workspaces.find(
      (w) => w.id === (activeWorkspaceId || workspaceQuery.data.activeWorkspaceId),
    ) ?? workspaceQuery.data?.workspaces[0];
  const workspaceLayoutQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-layout", activeWorkspace?.id],
    queryFn: () => getWorkspaceLayout(activeWorkspace?.id ?? ""),
  });
  const workspaceEnvironmentsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-environments", activeWorkspace?.id],
    queryFn: () => listWorkspaceEnvironments(activeWorkspace?.id ?? ""),
    refetchOnWindowFocus: true,
  });
  const sidebarDatabaseConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["database-connections", activeWorkspace?.id],
    queryFn: () => listDatabaseConnections(activeWorkspace?.id ?? ""),
  });
  useWorkspaceInit(workspaceQuery.data?.activeWorkspaceId, workspaceLayoutQuery.data, sidebarDatabaseConnectionsQuery.data);
  useLayoutPersistence(activeWorkspace?.id ?? null);
  const activateEnvironmentMutation = useMutation({
    mutationFn: (input: { environmentId: string | null; workspaceId: string }) =>
      setActiveWorkspaceEnvironment(input.workspaceId, input.environmentId),
    onSuccess: (environments, input) => {
      queryClient.setQueryData(["workspace-environments", input.workspaceId], environments);
    },
    onError: (error) => handleError(error, { key: "feedback.api.environmentActivateFailed" }),
  });
  const workspaceEnvironments = workspaceEnvironmentsQuery.data ?? [];
  const activeEnvironment = workspaceEnvironments.find((environment) => environment.isActive) ?? null;
  const selectEnvironment = (environmentId: string | null) =>
    activeWorkspace && activateEnvironmentMutation.mutate({ environmentId, workspaceId: activeWorkspace.id });
  const refreshWorkspaceEnvironments = useCallback(() => {
    if (!activeWorkspace?.id) return;
    void queryClient.refetchQueries({ queryKey: ["workspace-environments", activeWorkspace.id] });
  }, [activeWorkspace, queryClient]);
  const refreshWorkspaces = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
  }, [queryClient]);
  return {
    activeEnvironment, activeWorkspace, healthQuery, workspaceEnvironments, workspaceQuery,
    refreshWorkspaceEnvironments, refreshWorkspaces, selectEnvironment,
  };
}

export type DesktopWorkspaceController = ReturnType<typeof useDesktopWorkspace>;
