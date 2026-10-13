import { reorderMockCollectionRows } from "./api-collection-order";
import {
  assertMockCollection,
  assertMockFolder,
  mockState,
  mockStore,
  nextMockRequestSortOrder,
  normalizeMockId,
} from "./state";

export function moveMockRequest(args?: Record<string, unknown>) {
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
  return request;
}

export function reorderMockRequests(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const collectionId = String(args?.collectionId ?? "");
  const parentFolderId = normalizeMockId(args?.parentFolderId);
  const requestIds = Array.isArray(args?.requestIds)
    ? (args.requestIds as string[])
    : [];
  assertMockCollection(workspaceId, collectionId);
  assertMockFolder(workspaceId, collectionId, parentFolderId);
  reorderMockCollectionRows(
    mockStore.savedRequests, requestIds,
    { workspaceId, collectionId, parentFolderId }, "api request not found",
  );
  return mockStore.savedRequests.filter((item) => item.workspaceId === workspaceId);
}
