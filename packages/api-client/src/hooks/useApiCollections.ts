import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createApiCollection,
  deleteApiCollection,
  listApiCollections,
  renameApiCollection,
  type ApiCollection,
} from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";

/**
 * Shared CRUD for API collections. All mutations invalidate the
 * `["api-collections", workspaceId]` query; delete also invalidates
 * `["api-saved", workspaceId]` because the backend cascade soft-deletes the
 * collection's saved requests.
 */
export function useApiCollections(workspaceId: string) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();

  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-collections", workspaceId],
    queryFn: () => listApiCollections(workspaceId),
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["api-collections", workspaceId] });

  const createMut = useMutation({
    mutationFn: (name: string) => createApiCollection(workspaceId, name),
    onSuccess: invalidate,
    onError: (error) =>
      handleError(error, { key: "feedback.api.collectionCreateFailed" }),
  });
  const renameMut = useMutation({
    mutationFn: (input: { id: string; name: string }) =>
      renameApiCollection(workspaceId, input.id, input.name),
    onSuccess: invalidate,
    onError: (error) =>
      handleError(error, { key: "feedback.api.collectionRenameFailed" }),
  });
  const deleteMut = useMutation({
    mutationFn: (collectionId: string) =>
      deleteApiCollection(workspaceId, collectionId),
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({
        queryKey: ["api-collection-folders", workspaceId],
      });
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
    },
    onError: (error) =>
      handleError(error, { key: "feedback.api.collectionDeleteFailed" }),
  });
  const collections = useMemo<ApiCollection[]>(
    () => query.data ?? [],
    [query.data],
  );

  return {
    collections,
    createMut,
    deleteMut,
    isLoading: query.isLoading,
    renameMut,
  };
}
