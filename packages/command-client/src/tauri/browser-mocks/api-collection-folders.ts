import { reorderMockCollectionRows } from "./api-collection-order";
import {
  assertMockCollection,
  assertMockFolder,
  descendantFolderIds,
  mockFolderList,
  mockState,
  mockStore,
  nextMockFolderSortOrder,
  normalizeMockId,
} from "./state";
import type { ApiCollectionFolder } from "../../types";

export function listMockCollectionFolders(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const collectionId = normalizeMockId(args?.collectionId);
  return mockFolderList(workspaceId, collectionId);
}

export function createMockCollectionFolder(args?: Record<string, unknown>) {
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
  return folder;
}

export function renameMockCollectionFolder(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const folderId = String(args?.folderId ?? "");
  const folder = mockStore.collectionFolders.find(
    (item) => item.workspaceId === workspaceId && item.id === folderId,
  );
  if (!folder) throw new Error("api collection folder not found");
  folder.name = String(args?.name ?? folder.name).trim() || folder.name;
  folder.updatedAt = new Date().toISOString();
  return folder;
}

export function deleteMockCollectionFolder(args?: Record<string, unknown>) {
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
  return undefined;
}

export function moveMockCollectionFolder(args?: Record<string, unknown>) {
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
  return folder;
}

export function reorderMockCollectionFolders(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const collectionId = String(args?.collectionId ?? "");
  const parentFolderId = normalizeMockId(args?.parentFolderId);
  const folderIds = Array.isArray(args?.folderIds)
    ? (args.folderIds as string[])
    : [];
  assertMockCollection(workspaceId, collectionId);
  assertMockFolder(workspaceId, collectionId, parentFolderId);
  reorderMockCollectionRows(
    mockStore.collectionFolders, folderIds,
    { workspaceId, collectionId, parentFolderId }, "api collection folder not found",
  );
  return mockFolderList(workspaceId, collectionId);
}
