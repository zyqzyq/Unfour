import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  activateApiEnvironment,
  createApiEnvironment,
  deleteApiEnvironment,
  listApiEnvironments,
  updateApiEnvironment,
  type ApiEnvironment,
  type KeyValue,
} from "@unfour/command-client";

/**
 * Shared CRUD + activation for API environments. All mutations invalidate the
 * single `["api-environments", workspaceId]` query (the same key
 * `useApiRequestTabs` reads), so the request bar and the sidebar stay in sync.
 */
export function useApiEnvironments(workspaceId: string) {
  const queryClient = useQueryClient();

  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-environments", workspaceId],
    queryFn: () => listApiEnvironments(workspaceId),
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["api-environments", workspaceId] });

  const createMut = useMutation({
    mutationFn: (name: string) => createApiEnvironment(workspaceId, name),
    onSuccess: invalidate,
  });
  const updateMut = useMutation({
    mutationFn: (input: { id: string; name: string; variables: KeyValue[] }) =>
      updateApiEnvironment(workspaceId, input.id, input.name, input.variables),
    onSuccess: invalidate,
  });
  const deleteMut = useMutation({
    mutationFn: (environmentId: string) =>
      deleteApiEnvironment(workspaceId, environmentId),
    onSuccess: invalidate,
  });
  const activateMut = useMutation({
    mutationFn: (environmentId: string | null) =>
      activateApiEnvironment(workspaceId, environmentId),
    onSuccess: invalidate,
  });

  const environments = useMemo<ApiEnvironment[]>(
    () => query.data ?? [],
    [query.data],
  );
  const activeEnvironment = useMemo(
    () => environments.find((environment) => environment.isActive) ?? null,
    [environments],
  );

  return {
    activateMut,
    activeEnvironment,
    createMut,
    deleteMut,
    environments,
    isLoading: query.isLoading,
    updateMut,
  };
}
