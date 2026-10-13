import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApiCollection, ApiCollectionFolder, ApiSavedRequest } from "../../types";
import { handleApiCollectionMock } from "./api-collections";
import { mockInvoke } from "./index";
import { mockState, mockStore } from "./state";
import { UNHANDLED } from "./types";

const initial = structuredClone({
  active: mockState.activeWorkspaceId, collections: mockStore.collections,
  folders: mockStore.collectionFolders, requests: mockStore.savedRequests,
});
const createdAt = "2026-01-01T00:00:00.000Z";
const updatedAt = "2026-01-02T00:00:00.000Z";

beforeEach(() => {
  mockState.activeWorkspaceId = "w1";
  mockStore.collections = [];
  mockStore.collectionFolders = [];
  mockStore.savedRequests = [];
  vi.useFakeTimers();
  vi.setSystemTime(new Date(createdAt));
});

afterEach(() => {
  const restored = structuredClone(initial);
  mockState.activeWorkspaceId = restored.active;
  mockStore.collections = restored.collections;
  mockStore.collectionFolders = restored.folders;
  mockStore.savedRequests = restored.requests;
  vi.useRealTimers();
});

function collection(name = "Collection", workspaceId = "w1") {
  return mockInvoke<ApiCollection>("api_collection_create", { name, workspaceId });
}

function folder(owner: ApiCollection, parentFolderId: string | null = null, name = "Folder") {
  return mockInvoke<ApiCollectionFolder>("api_collection_folder_create", {
    workspaceId: owner.workspaceId, collectionId: owner.id, parentFolderId, name,
  });
}

function request(owner: ApiCollection, parentFolderId: string | null = null) {
  return mockInvoke<ApiSavedRequest>("api_request_save", { input: {
    workspaceId: owner.workspaceId, collectionId: owner.id, parentFolderId,
    method: "GET", url: "https://example.invalid", headers: [], query: [], bodyKind: "none",
  } });
}

describe("collection mock routing and lifecycle", () => {
  it("preserves browser cancellation results and unknown-command fallthrough", async () => {
    await expect(mockInvoke("api_collection_import_preview")).resolves.toBeNull();
    await expect(mockInvoke("api_collection_import")).resolves.toEqual({
      imported: false, collection: null, folderCount: 0, requestCount: 0,
    });
    await expect(mockInvoke("api_collection_export")).resolves.toEqual({ saved: false });
    await expect(mockInvoke("api_collection_export", { collectionId: "missing" })).rejects.toThrow("api collection not found");
    expect(handleApiCollectionMock("unknown")).toBe(UNHANDLED);
    expect(handleApiCollectionMock("toString")).toBe(UNHANDLED);
  });

  it("keeps active-workspace defaults, trimmed names, sorted lists and in-place rename", async () => {
    const a = await mockInvoke<ApiCollection>("api_collection_create");
    const b = await collection("  Alpha  ");
    await collection("Foreign", "w2");
    expect(a).toMatchObject({ workspaceId: "w1", name: "New Collection", description: null, createdAt, updatedAt: createdAt });
    expect(b.name).toBe("Alpha");
    await expect(mockInvoke("api_collection_list")).resolves.toEqual([b, a]);
    vi.setSystemTime(new Date(updatedAt));
    expect(await mockInvoke("api_collection_rename", { collectionId: a.id, name: " " })).toBe(a);
    expect(a).toMatchObject({ name: "New Collection", createdAt, updatedAt });
    await mockInvoke("api_collection_rename", { collectionId: a.id, name: "  Renamed  " });
    expect(a.name).toBe("Renamed");
    await expect(mockInvoke("api_collection_rename", { workspaceId: "w2", collectionId: a.id })).rejects.toThrow("api collection not found");
  });

  it("cascades collection deletion only within the workspace and collection", async () => {
    const a = await collection();
    const keep = await collection("Keep");
    const f = await folder(a);
    await request(a, f.id);
    const keepRequest = await request(keep);
    const foreign = { ...a, workspaceId: "w2" };
    mockStore.collections.push(foreign);
    const foreignFolder = { ...f, workspaceId: "w2" };
    mockStore.collectionFolders.push(foreignFolder);
    const foreignRequest = { ...mockStore.savedRequests[1], workspaceId: "w2" };
    mockStore.savedRequests.push(foreignRequest);
    await expect(mockInvoke("api_collection_delete", { collectionId: a.id })).resolves.toEqual([keep]);
    expect(mockStore.collections).toEqual([keep, foreign]);
    expect(mockStore.collectionFolders).toEqual([foreignFolder]);
    expect(mockStore.savedRequests).toEqual([keepRequest, foreignRequest]);
    await expect(mockInvoke("api_collection_delete", { collectionId: a.id })).rejects.toThrow("api collection not found");
  });
});

describe("collection folder hierarchy", () => {
  it("creates scoped siblings, updates collection time and preserves rename identity", async () => {
    const a = await collection();
    const root = await folder(a, null, " ");
    const second = await folder(a);
    vi.setSystemTime(new Date(updatedAt));
    const child = await folder(a, ` ${root.id} `, "  Child  ");
    expect(root).toMatchObject({ name: "New Folder", sortOrder: 0, deletedAt: null });
    expect(second.sortOrder).toBe(1);
    expect(child).toMatchObject({ name: "Child", parentFolderId: root.id, sortOrder: 0, createdAt: updatedAt });
    expect(a.updatedAt).toBe(updatedAt);
    expect(await mockInvoke("api_collection_folder_rename", { folderId: root.id, name: " " })).toBe(root);
    expect(root).toMatchObject({ name: "New Folder", createdAt, updatedAt });
    await mockInvoke("api_collection_folder_rename", { folderId: root.id, name: "  Parent  " });
    expect(root.name).toBe("Parent");
    await expect(mockInvoke("api_collection_folders_list", { collectionId: ` ${a.id} ` })).resolves.toEqual([root, second, child]);
    await expect(mockInvoke("api_collection_folder_create", { collectionId: "missing" })).rejects.toThrow("api collection not found");
    await expect(folder(a, "missing")).rejects.toThrow("api collection folder not found");
  });

  it("deletes descendants and their requests, preserving siblings and returning handled undefined", async () => {
    const a = await collection();
    const root = await folder(a);
    const child = await folder(a, root.id);
    const grandchild = await folder(a, child.id);
    const keep = await folder(a);
    await request(a, grandchild.id);
    const keepRequest = await request(a, keep.id);
    const foreign = { ...child, workspaceId: "w2" };
    mockStore.collectionFolders.push(foreign);
    await expect(mockInvoke("api_collection_folder_delete", { folderId: root.id })).resolves.toBeUndefined();
    expect(mockStore.collectionFolders).toEqual([keep, foreign]);
    expect(mockStore.savedRequests).toEqual([keepRequest]);
    await expect(mockInvoke("api_collection_folder_delete", { folderId: root.id })).rejects.toThrow("api collection folder not found");
  });

  it("rejects self/descendant/cross-collection moves before mutation", async () => {
    const a = await collection();
    const root = await folder(a);
    const child = await folder(a, root.id);
    const foreign = await folder(await collection("Other"));
    const before = structuredClone(mockStore.collectionFolders);
    for (const [targetParentFolderId, message] of [
      [root.id, "folder cannot be its own parent"],
      [child.id, "folder cannot be moved into its descendant"],
      [foreign.id, "api collection folder not found"],
    ]) {
      await expect(mockInvoke("api_collection_folder_move", { folderId: root.id, targetParentFolderId })).rejects.toThrow(message);
    }
    expect(mockStore.collectionFolders).toEqual(before);
    await expect(mockInvoke("api_collection_folder_move", { workspaceId: "w2", folderId: root.id })).rejects.toThrow("api collection folder not found");
  });

  it("preserves move ordering, including the moved row in the next-order calculation", async () => {
    const a = await collection();
    const root = await folder(a);
    const moving = await folder(a);
    moving.sortOrder = 9;
    vi.setSystemTime(new Date(updatedAt));
    expect(await mockInvoke("api_collection_folder_move", { folderId: moving.id, targetParentFolderId: ` ${root.id} ` })).toBe(moving);
    expect(moving).toMatchObject({ parentFolderId: root.id, sortOrder: 10, updatedAt });
    await mockInvoke("api_collection_folder_move", { folderId: moving.id, targetParentFolderId: " " });
    expect(moving).toMatchObject({ parentFolderId: null, sortOrder: 11 });
  });
});

describe("request organization and sibling ordering", () => {
  it("moves a request after validation, preserving its identity and body metadata", async () => {
    const a = await collection();
    const b = await collection("Other");
    const parent = await folder(b);
    const moving = await request(a);
    moving.sortOrder = 9;
    const before = structuredClone(moving);
    await expect(mockInvoke("api_request_move", { requestId: moving.id, collectionId: a.id, parentFolderId: parent.id })).rejects.toThrow("api collection folder not found");
    await expect(mockInvoke("api_request_move", { workspaceId: "w2", requestId: moving.id, collectionId: b.id })).rejects.toThrow("api request not found");
    expect(moving).toEqual(before);
    vi.setSystemTime(new Date(updatedAt));
    expect(await mockInvoke("api_request_move", { requestId: moving.id, collectionId: b.id, parentFolderId: ` ${parent.id} ` })).toBe(moving);
    expect(moving).toEqual({ ...before, collectionId: b.id, parentFolderId: parent.id, sortOrder: 10, updatedAt });
  });

  it.each(["folder", "request"] as const)("preserves partial/duplicate %s reorder and partial updates on a later invalid id", async (kind) => {
    const a = await collection();
    const create = kind === "folder" ? folder : request;
    const first = await create(a);
    const second = await create(a);
    const parent = await folder(a);
    const child = await create(a, parent.id);
    const other = await create(await collection("Other"));
    const foreign = await create(await collection("Foreign", "w2"));
    const command = kind === "folder" ? "api_collection_folders_reorder" : "api_requests_reorder";
    const idsKey = kind === "folder" ? "folderIds" : "requestIds";
    const reorder = (ids: unknown) => mockInvoke(command, { collectionId: a.id, [idsKey]: ids });
    vi.setSystemTime(new Date(updatedAt));
    await reorder([second.id, second.id, first.id]);
    expect([first.sortOrder, second.sortOrder]).toEqual([2, 1]);
    expect(first.updatedAt).toBe(updatedAt);
    expect(child.updatedAt).toBe(createdAt);
    const rows = structuredClone(kind === "folder" ? mockStore.collectionFolders : mockStore.savedRequests);
    await reorder(null);
    expect(kind === "folder" ? mockStore.collectionFolders : mockStore.savedRequests).toEqual(rows);
    const message = kind === "folder" ? "api collection folder not found" : "api request not found";
    for (const invalid of [child.id, other.id, foreign.id, "missing"]) {
      first.sortOrder = 9;
      await expect(reorder([first.id, invalid])).rejects.toThrow(message);
      expect(first.sortOrder).toBe(0);
    }
    const result = await reorder([second.id]);
    if (kind === "folder") {
      expect(result).toEqual([first, second, parent, child]);
    } else {
      // Request reorder returns all workspace requests in storage order.
      expect(result).toEqual([other, child, second, first]);
    }
  });
});
