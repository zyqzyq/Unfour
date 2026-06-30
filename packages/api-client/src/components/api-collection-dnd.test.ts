import type { ApiCollection, ApiCollectionFolder } from "@unfour/command-client";
import { describe, expect, it } from "vitest";
import { buildApiCollectionTree } from "../request-utils";
import { createApiCollectionDropController } from "./api-collection-dnd";

describe("createApiCollectionDropController", () => {
  it("allows sibling folder reordering", () => {
    const folders = [
      folder({ id: "folder-1", name: "Auth", sortOrder: 0 }),
      folder({ id: "folder-2", name: "Billing", sortOrder: 1 }),
    ];
    const groups = buildApiCollectionTree([collection()], folders, []);
    const controller = createApiCollectionDropController(groups, folders, []);

    expect(
      controller.dropAction(
        { id: "folder:folder-2", label: "Billing" },
        { id: "folder:folder-1", label: "Auth" },
        "before",
      ),
    ).toEqual({
      kind: "reorder-folders",
      collectionId: "col-1",
      folderIds: ["folder-2", "folder-1"],
      parentFolderId: null,
    });
  });

  it("allows moving a folder into another folder in the same collection", () => {
    const folders = [
      folder({ id: "folder-1", name: "Auth", sortOrder: 0 }),
      folder({ id: "folder-2", name: "Billing", sortOrder: 1 }),
    ];
    const groups = buildApiCollectionTree([collection()], folders, []);
    const controller = createApiCollectionDropController(groups, folders, []);

    expect(
      controller.dropAction(
        { id: "folder:folder-1", label: "Auth" },
        { id: "folder:folder-2", label: "Billing" },
        "inside",
      ),
    ).toEqual({
      kind: "move-folder",
      folderId: "folder-1",
      targetParentFolderId: "folder-2",
    });
  });

  it("does not allow moving a folder into its own child", () => {
    const folders = [
      folder({ id: "folder-1", name: "Auth", parentFolderId: null }),
      folder({ id: "folder-2", name: "Tokens", parentFolderId: "folder-1" }),
    ];
    const groups = buildApiCollectionTree([collection()], folders, []);
    const controller = createApiCollectionDropController(groups, folders, []);

    expect(
      controller.dropAction(
        { id: "folder:folder-1", label: "Auth" },
        { id: "folder:folder-2", label: "Tokens" },
        "inside",
      ),
    ).toBeNull();
  });
});

function collection(overrides: Partial<ApiCollection> = {}): ApiCollection {
  return {
    id: "col-1",
    workspaceId: "ws-1",
    name: "Users",
    description: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function folder(overrides: Partial<ApiCollectionFolder> = {}): ApiCollectionFolder {
  return {
    id: "folder-1",
    workspaceId: "ws-1",
    collectionId: "col-1",
    parentFolderId: null,
    name: "Auth",
    sortOrder: 0,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
    ...overrides,
  };
}
