import { useState } from "react";
import type { ApiCollection, ApiSavedRequest } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { findDuplicateRequestName, type FolderNode } from "../request-utils";
import type { useApiCollections } from "./useApiCollections";
import type { useApiCollectionFolders } from "./useApiCollectionFolders";
import type { useApiCollectionRequestActions } from "./useApiCollectionRequestActions";

type NameTarget =
  | { kind: "collection" }
  | { kind: "folder"; collectionId: string; parentFolderId: string | null };

function useNameDialog<T>() {
  const [target, setTarget] = useState<T | null>(null);
  const [value, setValue] = useState("");
  return {
    target,
    value,
    onChange: setValue,
    onClose: () => setTarget(null),
    open: (next: T, name: string) => { setTarget(next); setValue(name); },
  };
}

export function useApiCollectionTreeDialogs({
  collections,
  folders,
  requestActions,
  requests,
}: {
  collections: ReturnType<typeof useApiCollections>;
  folders: ReturnType<typeof useApiCollectionFolders>;
  requestActions: ReturnType<typeof useApiCollectionRequestActions>;
  requests: ApiSavedRequest[];
}) {
  const { t } = useI18n();
  const name = useNameDialog<NameTarget>();
  const rename = useNameDialog<ApiCollection>();
  const renameFolder = useNameDialog<FolderNode>();
  const renameRequest = useNameDialog<ApiSavedRequest>();
  const [deleteTarget, setDeleteTarget] = useState<ApiCollection | null>(null);
  const [deleteFolderTarget, setDeleteFolderTarget] = useState<FolderNode | null>(null);
  const [exportTarget, setExportTarget] = useState<ApiCollection | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function confirmName() {
    const value = name.value.trim();
    if (!name.target || !value) return;
    if (name.target.kind === "collection") {
      collections.createMut.mutate(value, { onSuccess: name.onClose });
      return;
    }
    folders.createFolderMut.mutate(
      { collectionId: name.target.collectionId, name: value, parentFolderId: name.target.parentFolderId },
      { onSuccess: name.onClose },
    );
  }

  function confirmRename() {
    const value = rename.value.trim();
    if (!rename.target || !value) return;
    collections.renameMut.mutate(
      { id: rename.target.id, name: value },
      { onSuccess: rename.onClose },
    );
  }

  function confirmFolderRename() {
    const value = renameFolder.value.trim();
    if (!renameFolder.target || !value) return;
    folders.renameFolderMut.mutate(
      { folderId: renameFolder.target.id, name: value },
      { onSuccess: renameFolder.onClose },
    );
  }

  function confirmRequestRename() {
    const value = renameRequest.value.trim();
    const request = renameRequest.target;
    if (!request || !value) return;
    if (findDuplicateRequestName(requests, value, request.collectionId, request.parentFolderId, request.id)) {
      setErrorMessage(t("api.save.duplicateName", { name: value }));
      return;
    }
    requestActions.updateRequestMutation.mutate(
      { name: value, request },
      { onSuccess: renameRequest.onClose },
    );
  }

  function confirmDelete() {
    if (!deleteTarget) return;
    collections.deleteMut.mutate(deleteTarget.id, { onSuccess: () => setDeleteTarget(null) });
  }

  function confirmFolderDelete() {
    if (!deleteFolderTarget) return;
    folders.deleteFolderMut.mutate(deleteFolderTarget.id, { onSuccess: () => setDeleteFolderTarget(null) });
  }

  return {
    name: { ...name, confirm: confirmName, pending: collections.createMut.isPending || folders.createFolderMut.isPending },
    rename: { ...rename, confirm: confirmRename, pending: collections.renameMut.isPending },
    renameFolder: { ...renameFolder, confirm: confirmFolderRename, pending: folders.renameFolderMut.isPending },
    renameRequest: { ...renameRequest, confirm: confirmRequestRename, pending: requestActions.updateRequestMutation.isPending },
    deleteCollection: { target: deleteTarget, onClose: () => setDeleteTarget(null), confirm: confirmDelete, pending: collections.deleteMut.isPending },
    deleteFolder: { target: deleteFolderTarget, onClose: () => setDeleteFolderTarget(null), confirm: confirmFolderDelete, pending: folders.deleteFolderMut.isPending },
    exportCollection: { target: exportTarget, onClose: () => setExportTarget(null) },
    error: { message: errorMessage, onClose: () => setErrorMessage(null) },
    showError: setErrorMessage,
    openCollection: () => name.open({ kind: "collection" }, ""),
    openFolder: (collectionId: string, parentFolderId: string | null) =>
      name.open({ kind: "folder", collectionId, parentFolderId }, ""),
    renameCollection: (collection: ApiCollection) => rename.open(collection, collection.name),
    renameFolderItem: (folder: FolderNode) => renameFolder.open(folder, folder.name),
    renameRequestItem: (request: ApiSavedRequest) => renameRequest.open(request, request.name),
    deleteCollectionItem: setDeleteTarget,
    deleteFolderItem: setDeleteFolderTarget,
    exportCollectionItem: setExportTarget,
  };
}
