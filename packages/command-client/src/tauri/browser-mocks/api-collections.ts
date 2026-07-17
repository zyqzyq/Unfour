import {
  assertMockCollection,
  assertMockFolder,
  descendantFolderIds,
  mockCollectionList,
  mockFolderList,
  mockState,
  mockStore,
  nextMockFolderSortOrder,
  nextMockRequestSortOrder,
  normalizeMockId,
} from "./state";
import { UNHANDLED, type MockResult } from "./types";
import type { ApiCollection, ApiCollectionFolder } from "../../types";

export function handleApiCollectionMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "api_collection_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockCollectionList(workspaceId) as T;
  }

  if (command === "api_collection_export") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    assertMockCollection(workspaceId, collectionId);
    // Browser development has no Rust domain exporter or native save dialog.
    // Treat it like a cancelled dialog instead of duplicating export logic here.
    return { saved: false } as T;
  }

  if (command === "api_collection_import") {
    // Browser development has no native file picker or Rust importer.
    return {
      imported: false,
      collection: null,
      folderCount: 0,
      requestCount: 0,
    } as T;
  }

  if (command === "api_collection_create") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const now = new Date().toISOString();
    const collection: ApiCollection = {
      id: crypto.randomUUID(),
      workspaceId,
      name: String(args?.name ?? "New Collection").trim() || "New Collection",
      description: null,
      createdAt: now,
      updatedAt: now,
    };
    mockStore.collections = [...mockStore.collections, collection];
    return collection as T;
  }

  if (command === "api_collection_rename") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const collection = mockStore.collections.find(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!collection) throw new Error("api collection not found");
    collection.name = String(args?.name ?? collection.name).trim() || collection.name;
    collection.updatedAt = new Date().toISOString();
    return collection as T;
  }

  if (command === "api_collection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const existed = mockStore.collections.some(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!existed) throw new Error("api collection not found");
    mockStore.collections = mockStore.collections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === collectionId),
    );
    // Cascade: drop the collection's folder rows and saved requests.
    mockStore.collectionFolders = mockStore.collectionFolders.filter(
      (item) => !(item.workspaceId === workspaceId && item.collectionId === collectionId),
    );
    mockStore.savedRequests = mockStore.savedRequests.filter(
      (item) => !(item.workspaceId === workspaceId && item.collectionId === collectionId),
    );
    return mockCollectionList(workspaceId) as T;
  }

  if (command === "api_collection_folders_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = normalizeMockId(args?.collectionId);
    return mockFolderList(workspaceId, collectionId) as T;
  }

  if (command === "api_collection_folder_create") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const parentFolderId = normalizeMockId(args?.parentFolderId);
    const name = String(args?.name ?? "").trim() || "New Folder";
    const collection = mockStore.collections.find(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!collection) throw new Error("api collection not found");
    assertMockFolder(workspaceId, collectionId, parentFolderId);
    const now = new Date().toISOString();
    const folder: ApiCollectionFolder = {
      id: crypto.randomUUID(),
      workspaceId,
      collectionId,
      parentFolderId,
      name,
      sortOrder: nextMockFolderSortOrder(workspaceId, collectionId, parentFolderId),
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    };
    mockStore.collectionFolders = [...mockStore.collectionFolders, folder];
    collection.updatedAt = now;
    return folder as T;
  }

  if (command === "api_collection_folder_rename") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const folderId = String(args?.folderId ?? "");
    const folder = mockStore.collectionFolders.find(
      (item) => item.workspaceId === workspaceId && item.id === folderId,
    );
    if (!folder) throw new Error("api collection folder not found");
    folder.name = String(args?.name ?? folder.name).trim() || folder.name;
    folder.updatedAt = new Date().toISOString();
    return folder as T;
  }

  if (command === "api_collection_folder_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const folderId = String(args?.folderId ?? "");
    const folder = mockStore.collectionFolders.find(
      (item) => item.workspaceId === workspaceId && item.id === folderId,
    );
    if (!folder) throw new Error("api collection folder not found");
    const ids = descendantFolderIds(workspaceId, folderId);
    mockStore.collectionFolders = mockStore.collectionFolders.filter(
      (item) => !(item.workspaceId === workspaceId && ids.has(item.id)),
    );
    mockStore.savedRequests = mockStore.savedRequests.filter(
      (item) => !(item.workspaceId === workspaceId && ids.has(item.parentFolderId ?? "")),
    );
    return undefined as T;
  }

  if (command === "api_collection_folder_move") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const folderId = String(args?.folderId ?? "");
    const targetParentFolderId = normalizeMockId(args?.targetParentFolderId);
    const folder = mockStore.collectionFolders.find(
      (item) => item.workspaceId === workspaceId && item.id === folderId,
    );
    if (!folder) throw new Error("api collection folder not found");
    if (targetParentFolderId === folderId) {
      throw new Error("folder cannot be its own parent");
    }
    assertMockFolder(workspaceId, folder.collectionId, targetParentFolderId);
    if (
      targetParentFolderId &&
      descendantFolderIds(workspaceId, folderId).has(targetParentFolderId)
    ) {
      throw new Error("folder cannot be moved into its descendant");
    }
    folder.parentFolderId = targetParentFolderId;
    folder.sortOrder = nextMockFolderSortOrder(
      workspaceId,
      folder.collectionId,
      targetParentFolderId,
    );
    folder.updatedAt = new Date().toISOString();
    return folder as T;
  }

  if (command === "api_collection_folders_reorder") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const parentFolderId = normalizeMockId(args?.parentFolderId);
    const folderIds = Array.isArray(args?.folderIds)
      ? (args.folderIds as string[])
      : [];
    assertMockCollection(workspaceId, collectionId);
    assertMockFolder(workspaceId, collectionId, parentFolderId);
    folderIds.forEach((folderId, index) => {
      const folder = mockStore.collectionFolders.find(
        (item) =>
          item.workspaceId === workspaceId &&
          item.collectionId === collectionId &&
          item.parentFolderId === parentFolderId &&
          item.id === folderId,
      );
      if (!folder) throw new Error("api collection folder not found");
      folder.sortOrder = index;
      folder.updatedAt = new Date().toISOString();
    });
    return mockFolderList(workspaceId, collectionId) as T;
  }

  if (command === "api_request_move") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const requestId = String(args?.requestId ?? "");
    const collectionId = String(args?.collectionId ?? "");
    const parentFolderId = normalizeMockId(args?.parentFolderId);
    const request = mockStore.savedRequests.find(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (!request) throw new Error("api request not found");
    assertMockCollection(workspaceId, collectionId);
    assertMockFolder(workspaceId, collectionId, parentFolderId);
    request.collectionId = collectionId;
    request.parentFolderId = parentFolderId;
    request.sortOrder = nextMockRequestSortOrder(workspaceId, collectionId, parentFolderId);
    request.updatedAt = new Date().toISOString();
    return request as T;
  }

  if (command === "api_requests_reorder") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const parentFolderId = normalizeMockId(args?.parentFolderId);
    const requestIds = Array.isArray(args?.requestIds)
      ? (args.requestIds as string[])
      : [];
    assertMockCollection(workspaceId, collectionId);
    assertMockFolder(workspaceId, collectionId, parentFolderId);
    requestIds.forEach((requestId, index) => {
      const request = mockStore.savedRequests.find(
        (item) =>
          item.workspaceId === workspaceId &&
          item.collectionId === collectionId &&
          item.parentFolderId === parentFolderId &&
          item.id === requestId,
      );
      if (!request) throw new Error("api request not found");
      request.sortOrder = index;
      request.updatedAt = new Date().toISOString();
    });
    return mockStore.savedRequests.filter((item) => item.workspaceId === workspaceId) as T;
  }

  return UNHANDLED;
}
