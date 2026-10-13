import {
  assertMockCollection,
  mockCollectionList,
  mockState,
  mockStore,
} from "./state";
import type { ApiCollection } from "../../types";

export function listMockCollections(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  return mockCollectionList(workspaceId);
}

export function exportMockCollection(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const collectionId = String(args?.collectionId ?? "");
  assertMockCollection(workspaceId, collectionId);
  // Browser development has no Rust domain exporter or native save dialog.
  // Treat it like a cancelled dialog instead of duplicating export logic here.
  return { saved: false };
}

export function importMockCollection() {
  // Browser development has no native file picker or Rust importer.
  return {
    imported: false,
    collection: null,
    folderCount: 0,
    requestCount: 0,
  };
}

export function createMockCollection(args?: Record<string, unknown>) {
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
  return collection;
}

export function renameMockCollection(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const collectionId = String(args?.collectionId ?? "");
  const collection = mockStore.collections.find(
    (item) => item.workspaceId === workspaceId && item.id === collectionId,
  );
  if (!collection) throw new Error("api collection not found");
  collection.name = String(args?.name ?? collection.name).trim() || collection.name;
  collection.updatedAt = new Date().toISOString();
  return collection;
}

export function deleteMockCollection(args?: Record<string, unknown>) {
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
  return mockCollectionList(workspaceId);
}
