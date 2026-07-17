import { call } from "./invoke";
import type {
  ApiCollection,
  ApiCollectionExportFormat,
  ApiCollectionExportResult,
  ApiCollectionFolder,
  ApiCollectionImportResult,
  ApiEnvironment,
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
  KeyValue,
} from "../types";

export function listApiEnvironments(workspaceId: string) {
  return call<ApiEnvironment[]>("api_environments_list", { workspaceId });
}

export function createApiEnvironment(workspaceId: string, name: string) {
  return call<ApiEnvironment>("api_environment_create", { workspaceId, name });
}

export function updateApiEnvironment(
  workspaceId: string,
  environmentId: string,
  name: string,
  variables: KeyValue[],
) {
  return call<ApiEnvironment>("api_environment_update", {
    workspaceId,
    environmentId,
    name,
    variables,
  });
}

export function deleteApiEnvironment(workspaceId: string, environmentId: string) {
  return call<ApiEnvironment[]>("api_environment_delete", {
    workspaceId,
    environmentId,
  });
}

export function activateApiEnvironment(
  workspaceId: string,
  environmentId: string | null,
) {
  return call<ApiEnvironment[]>("api_environment_activate", {
    workspaceId,
    environmentId,
  });
}

export function listApiCollections(workspaceId: string) {
  return call<ApiCollection[]>("api_collection_list", { workspaceId });
}

export function exportApiCollection(
  workspaceId: string,
  collectionId: string,
  format: ApiCollectionExportFormat,
) {
  return call<ApiCollectionExportResult>("api_collection_export", {
    workspaceId,
    collectionId,
    format,
  });
}

export function importApiCollection(workspaceId: string) {
  return call<ApiCollectionImportResult>("api_collection_import", { workspaceId });
}

export function createApiCollection(workspaceId: string, name: string) {
  return call<ApiCollection>("api_collection_create", { workspaceId, name });
}

export function renameApiCollection(
  workspaceId: string,
  collectionId: string,
  name: string,
) {
  return call<ApiCollection>("api_collection_rename", {
    workspaceId,
    collectionId,
    name,
  });
}

export function deleteApiCollection(workspaceId: string, collectionId: string) {
  return call<ApiCollection[]>("api_collection_delete", {
    workspaceId,
    collectionId,
  });
}

export function listApiCollectionFolders(
  workspaceId: string,
  collectionId?: string | null,
) {
  return call<ApiCollectionFolder[]>("api_collection_folders_list", {
    workspaceId,
    collectionId: collectionId ?? null,
  });
}

export function createApiCollectionFolder(
  workspaceId: string,
  collectionId: string,
  parentFolderId: string | null,
  name: string,
) {
  return call<ApiCollectionFolder>("api_collection_folder_create", {
    workspaceId,
    collectionId,
    parentFolderId,
    name,
  });
}

export function renameApiCollectionFolder(
  workspaceId: string,
  folderId: string,
  name: string,
) {
  return call<ApiCollectionFolder>("api_collection_folder_rename", {
    workspaceId,
    folderId,
    name,
  });
}

export function deleteApiCollectionFolder(workspaceId: string, folderId: string) {
  return call<void>("api_collection_folder_delete", {
    workspaceId,
    folderId,
  });
}

export function moveApiCollectionFolder(
  workspaceId: string,
  folderId: string,
  targetParentFolderId: string | null,
) {
  return call<ApiCollectionFolder>("api_collection_folder_move", {
    workspaceId,
    folderId,
    targetParentFolderId,
  });
}

export function reorderApiCollectionFolders(
  workspaceId: string,
  collectionId: string,
  parentFolderId: string | null,
  folderIds: string[],
) {
  return call<ApiCollectionFolder[]>("api_collection_folders_reorder", {
    workspaceId,
    collectionId,
    parentFolderId,
    folderIds,
  });
}

export function moveApiRequest(
  workspaceId: string,
  requestId: string,
  collectionId: string,
  parentFolderId: string | null,
) {
  return call<ApiSavedRequest>("api_request_move", {
    workspaceId,
    requestId,
    collectionId,
    parentFolderId,
  });
}

export function reorderApiRequests(
  workspaceId: string,
  collectionId: string,
  parentFolderId: string | null,
  requestIds: string[],
) {
  return call<ApiSavedRequest[]>("api_requests_reorder", {
    workspaceId,
    collectionId,
    parentFolderId,
    requestIds,
  });
}

export function sendApiRequest(input: ApiRequestInput) {
  return call<ApiResponse>("api_send_request", { input });
}

export function saveApiRequest(input: ApiRequestInput) {
  return call<ApiSavedRequest>("api_request_save", { input });
}

export function updateApiRequest(
  workspaceId: string,
  requestId: string,
  input: ApiRequestInput,
) {
  return call<ApiSavedRequest>("api_request_update", {
    workspaceId,
    requestId,
    input,
  });
}

export function duplicateApiRequest(workspaceId: string, requestId: string) {
  return call<ApiSavedRequest>("api_request_duplicate", { workspaceId, requestId });
}

export function deleteApiRequest(workspaceId: string, requestId: string) {
  return call<ApiSavedRequest[]>("api_request_delete", { workspaceId, requestId });
}

export function listApiHistory(workspaceId: string) {
  return call<ApiHistoryItem[]>("api_history_list", { workspaceId, limit: 50 });
}

export function getApiHistoryDetail(workspaceId: string, historyId: string) {
  return call<ApiHistoryDetail>("api_history_detail", { workspaceId, historyId });
}

export function listSavedApiRequests(workspaceId: string) {
  return call<ApiSavedRequest[]>("api_saved_requests", { workspaceId });
}
