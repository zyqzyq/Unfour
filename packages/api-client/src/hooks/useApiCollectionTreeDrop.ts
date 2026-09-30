import type { ApiSavedRequest } from "@unfour/command-client";
import { useI18n, type TreeViewDropPosition, type TreeViewItem } from "@unfour/ui";
import { findDuplicateRequestName, type ApiCollectionGroup } from "../request-utils";
import { createApiCollectionDropController } from "../components/api-collection-dnd";
import type { useApiCollectionFolders } from "./useApiCollectionFolders";

export function useApiCollectionTreeDrop({
  collectionGroups,
  folderActions,
  requests,
  visibleRequests,
  onError,
}: {
  collectionGroups: ApiCollectionGroup[];
  folderActions: ReturnType<typeof useApiCollectionFolders>;
  requests: ApiSavedRequest[];
  visibleRequests: ApiSavedRequest[];
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const controller = createApiCollectionDropController(
    collectionGroups,
    folderActions.folders,
    visibleRequests,
  );

  function onDrop({ source, target, position }: {
    source: TreeViewItem;
    target: TreeViewItem;
    position: TreeViewDropPosition;
  }) {
    const action = controller.dropAction(source, target, position);
    if (!action) return;
    switch (action.kind) {
      case "move-folder":
        folderActions.moveFolderMut.mutate({
          folderId: action.folderId,
          targetParentFolderId: action.targetParentFolderId,
        });
        break;
      case "move-request": {
        const request = visibleRequests.find((item) => item.id === action.requestId);
        if (request && findDuplicateRequestName(
          requests, request.name, action.collectionId, action.parentFolderId, action.requestId,
        )) {
          onError(t("api.collection.moveDuplicateName", { name: request.name }));
          return;
        }
        folderActions.moveRequestMut.mutate({
          collectionId: action.collectionId,
          parentFolderId: action.parentFolderId,
          requestId: action.requestId,
        });
        break;
      }
      case "reorder-folders":
        folderActions.reorderFoldersMut.mutate({
          collectionId: action.collectionId,
          folderIds: action.folderIds,
          parentFolderId: action.parentFolderId,
        });
        break;
      case "reorder-requests":
        folderActions.reorderRequestsMut.mutate({
          collectionId: action.collectionId,
          parentFolderId: action.parentFolderId,
          requestIds: action.requestIds,
        });
        break;
    }
  }

  return { canDrop: controller.canDrop, onDrop };
}
