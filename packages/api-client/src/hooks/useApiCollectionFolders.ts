import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createApiCollectionFolder,
  deleteApiCollectionFolder,
  listApiCollectionFolders,
  moveApiCollectionFolder,
  moveApiRequest,
  renameApiCollectionFolder,
  reorderApiCollectionFolders,
  reorderApiRequests,
  type ApiCollectionFolder,
} from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";

export function useApiCollectionFolders(workspaceId: string) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();

  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-collection-folders", workspaceId],
    queryFn: () => listApiCollectionFolders(workspaceId),
  });

  const invalidateFolders = () =>
    queryClient.invalidateQueries({
      queryKey: ["api-collection-folders", workspaceId],
    });
  const invalidateSaved = () =>
    queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });

  const createFolderMut = useMutation({
    mutationFn: (input: {
      collectionId: string;
      name: string;
      parentFolderId: string | null;
    }) =>
      createApiCollectionFolder(
        workspaceId,
        input.collectionId,
        input.parentFolderId,
        input.name,
      ),
    onSuccess: invalidateFolders,
    onError: (error) => handleError(error, { key: "feedback.api.folderCreateFailed" }),
  });

  const renameFolderMut = useMutation({
    mutationFn: (input: { folderId: string; name: string }) =>
      renameApiCollectionFolder(workspaceId, input.folderId, input.name),
    onSuccess: invalidateFolders,
    onError: (error) => handleError(error, { key: "feedback.api.folderRenameFailed" }),
  });

  const deleteFolderMut = useMutation({
    mutationFn: (folderId: string) => deleteApiCollectionFolder(workspaceId, folderId),
    onSuccess: () => {
      invalidateFolders();
      invalidateSaved();
    },
    onError: (error) => handleError(error, { key: "feedback.api.folderDeleteFailed" }),
  });

  const moveFolderMut = useMutation({
    mutationFn: (input: {
      folderId: string;
      targetParentFolderId: string | null;
    }) =>
      moveApiCollectionFolder(
        workspaceId,
        input.folderId,
        input.targetParentFolderId,
      ),
    onSuccess: invalidateFolders,
    onError: (error) => handleError(error, { key: "feedback.api.folderMoveFailed" }),
  });

  const reorderFoldersMut = useMutation({
    mutationFn: (input: {
      collectionId: string;
      folderIds: string[];
      parentFolderId: string | null;
    }) =>
      reorderApiCollectionFolders(
        workspaceId,
        input.collectionId,
        input.parentFolderId,
        input.folderIds,
      ),
    onSuccess: invalidateFolders,
    onError: (error) => handleError(error, { key: "feedback.api.folderMoveFailed" }),
  });

  const moveRequestMut = useMutation({
    mutationFn: (input: {
      collectionId: string;
      parentFolderId: string | null;
      requestId: string;
    }) =>
      moveApiRequest(
        workspaceId,
        input.requestId,
        input.collectionId,
        input.parentFolderId,
      ),
    onSuccess: invalidateSaved,
    onError: (error) => handleError(error, { key: "feedback.api.requestMoveFailed" }),
  });

  const reorderRequestsMut = useMutation({
    mutationFn: (input: {
      collectionId: string;
      parentFolderId: string | null;
      requestIds: string[];
    }) =>
      reorderApiRequests(
        workspaceId,
        input.collectionId,
        input.parentFolderId,
        input.requestIds,
      ),
    onSuccess: invalidateSaved,
    onError: (error) => handleError(error, { key: "feedback.api.requestMoveFailed" }),
  });

  const folders = useMemo<ApiCollectionFolder[]>(
    () => query.data ?? [],
    [query.data],
  );

  return {
    createFolderMut,
    deleteFolderMut,
    folders,
    isLoading: query.isLoading,
    moveFolderMut,
    moveRequestMut,
    renameFolderMut,
    reorderFoldersMut,
    reorderRequestsMut,
  };
}
