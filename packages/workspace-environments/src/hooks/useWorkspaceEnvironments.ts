import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createWorkspaceEnvironment,
  deleteWorkspaceEnvironment,
  listWorkspaceEnvironments,
  setActiveWorkspaceEnvironment,
  updateWorkspaceEnvironmentVariables,
  type WorkspaceEnvironment,
  type WorkspaceVariableInput,
} from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";

export function useWorkspaceEnvironments(workspaceId: string) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const queryKey = ["workspace-environments", workspaceId] as const;
  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey,
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["environments"] },
    queryFn: () => listWorkspaceEnvironments(workspaceId),
  });

  const mergeEnvironment = async (saved: WorkspaceEnvironment) => {
    await queryClient.cancelQueries({ queryKey });
    queryClient.setQueryData<WorkspaceEnvironment[]>(queryKey, (current = []) => {
      const index = current.findIndex((environment) => environment.id === saved.id);
      if (index < 0) return [...current, saved];
      return current.map((environment) =>
        environment.id === saved.id ? saved : environment,
      );
    });
  };
  const createMut = useMutation({
    mutationFn: (name: string) => createWorkspaceEnvironment(workspaceId, name),
    // Use mutation results only — do not invalidate here. create→update save
    // can race a stale list refetch and wipe variables just written by update.
    onSuccess: mergeEnvironment,
  });
  const updateMut = useMutation({
    mutationFn: (input: {
      id: string;
      name: string;
      variables: WorkspaceVariableInput[];
      expected?: WorkspaceEnvironment;
    }) =>
      updateWorkspaceEnvironmentVariables(
        workspaceId,
        input.id,
        input.name,
        input.variables,
        input.expected,
      ),
    onSuccess: mergeEnvironment,
    onError: () => queryClient.invalidateQueries({ queryKey }),
  });
  const deleteMut = useMutation({
    mutationFn: (environmentId: string) =>
      deleteWorkspaceEnvironment(workspaceId, environmentId),
    onSuccess: (saved) => queryClient.setQueryData(queryKey, saved),
    onError: (error) =>
      handleError(error, { key: "feedback.api.environmentDeleteFailed" }),
  });
  const activateMut = useMutation({
    mutationKey: ["workspace-environment-activation", workspaceId],
    scope: { id: `workspace-environment-activation:${workspaceId}` },
    onMutate: () => ({ workspaceId }),
    mutationFn: (environmentId: string | null) =>
      setActiveWorkspaceEnvironment(workspaceId, environmentId),
    onSuccess: async (saved, _id, context) => {
      const key = ["workspace-environments", context.workspaceId];
      await queryClient.cancelQueries({ queryKey: key });
      queryClient.setQueryData(key, saved);
    },
    onError: (error) =>
      handleError(error, { key: "feedback.api.environmentActivateFailed" }),
  });

  const environments = useMemo<WorkspaceEnvironment[]>(
    () => query.data ?? [],
    [query.data],
  );

  return {
    activateMut,
    activeEnvironment:
      environments.find((environment) => environment.isActive) ?? null,
    createMut,
    deleteMut,
    environments,
    isLoading: query.isLoading,
    updateMut,
  };
}
