import type { ApiCollectionFolder, ApiSavedRequest } from "@unfour/command-client";
import type { TreeViewDropPosition, TreeViewItem } from "@unfour/ui";
import type { ApiCollectionGroup, FolderNode } from "../request-utils";

type DropLocation = {
  collectionId: string;
  parentFolderId: string | null;
};

type DropIndex = {
  folderById: Map<string, ApiCollectionFolder>;
  requestById: Map<string, ApiSavedRequest>;
  requestLocationById: Map<string, DropLocation>;
  requestSiblingIdsByLocation: Map<string, string[]>;
};

export type ApiCollectionDropAction =
  | {
      kind: "move-request";
      collectionId: string;
      parentFolderId: string | null;
      requestId: string;
    }
  | {
      kind: "reorder-requests";
      collectionId: string;
      parentFolderId: string | null;
      requestIds: string[];
    };

export function createApiCollectionDropController(
  collectionGroups: ApiCollectionGroup[],
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
) {
  const index = buildDropIndex(collectionGroups, folders, requests);

  return {
    canDrop: (
      source: TreeViewItem,
      target: TreeViewItem,
      position: TreeViewDropPosition,
    ) => Boolean(dropActionFor(index, source, target, position)),
    dropAction: (
      source: TreeViewItem,
      target: TreeViewItem,
      position: TreeViewDropPosition,
    ) => dropActionFor(index, source, target, position),
  };
}

function buildDropIndex(
  collectionGroups: ApiCollectionGroup[],
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
): DropIndex {
  const index: DropIndex = {
    folderById: new Map(folders.map((folder) => [folder.id, folder])),
    requestById: new Map(requests.map((request) => [request.id, request])),
    requestLocationById: new Map(),
    requestSiblingIdsByLocation: new Map(),
  };

  for (const group of collectionGroups) {
    const rootLocation = {
      collectionId: group.id,
      parentFolderId: null,
    };
    index.requestSiblingIdsByLocation.set(
      locationKey(rootLocation),
      group.tree.rootRequests.map((request) => request.id),
    );
    for (const request of group.tree.rootRequests) {
      index.requestLocationById.set(request.id, rootLocation);
    }
    for (const folder of group.tree.folders) {
    indexFolder(index, folder);
    }
  }

  return index;
}

function indexFolder(index: DropIndex, folder: FolderNode) {
  for (const request of folder.requests) {
    index.requestLocationById.set(request.id, {
      collectionId: folder.collectionId,
      parentFolderId: folder.id,
    });
  }

  const childLocation = {
    collectionId: folder.collectionId,
    parentFolderId: folder.id,
  };
  index.requestSiblingIdsByLocation.set(
    locationKey(childLocation),
    folder.requests.map((request) => request.id),
  );
  for (const child of folder.folders) {
    indexFolder(index, child);
  }
}

function dropActionFor(
  index: DropIndex,
  source: TreeViewItem,
  target: TreeViewItem,
  position: TreeViewDropPosition,
): ApiCollectionDropAction | null {
  if (source.id.startsWith("request:")) {
    return requestDropAction(index, source.id.slice("request:".length), target, position);
  }
  return null;
}

function requestDropAction(
  index: DropIndex,
  requestId: string,
  target: TreeViewItem,
  position: TreeViewDropPosition,
): ApiCollectionDropAction | null {
  const request = index.requestById.get(requestId);
  if (!request) {
    return null;
  }

  if (position === "inside") {
    const targetLocation = dropTargetLocation(index, target.id);
    if (!targetLocation || sameLocation(request, targetLocation)) {
      return null;
    }
    return {
      kind: "move-request",
      collectionId: targetLocation.collectionId,
      parentFolderId: targetLocation.parentFolderId,
      requestId,
    };
  }

  const targetRequestId = target.id.startsWith("request:")
    ? target.id.slice("request:".length)
    : null;
  const targetLocation = targetRequestId
    ? index.requestLocationById.get(targetRequestId)
    : null;
  const sourceLocation = index.requestLocationById.get(requestId);
  if (!targetRequestId || !targetLocation || !sourceLocation) {
    return null;
  }
  if (!sameLocation(sourceLocation, targetLocation)) {
    return null;
  }
  return reorderRequestAction(
    index.requestSiblingIdsByLocation.get(locationKey(targetLocation)) ?? [],
    requestId,
    targetRequestId,
    position,
    targetLocation,
  );
}

function reorderRequestAction(
  siblingIds: string[],
  sourceId: string,
  targetId: string,
  position: "before" | "after",
  location: DropLocation,
): ApiCollectionDropAction | null {
  const nextIds = siblingIds.filter((id) => id !== sourceId);
  const targetIndex = nextIds.indexOf(targetId);
  if (targetIndex < 0) {
    return null;
  }
  nextIds.splice(position === "before" ? targetIndex : targetIndex + 1, 0, sourceId);
  if (sameOrder(siblingIds, nextIds)) {
    return null;
  }
  return {
    kind: "reorder-requests",
    collectionId: location.collectionId,
    parentFolderId: location.parentFolderId,
    requestIds: nextIds,
  };
}

function dropTargetLocation(index: DropIndex, itemId: string): DropLocation | null {
  if (itemId.startsWith("collection:")) {
    return {
      collectionId: itemId.slice("collection:".length),
      parentFolderId: null,
    };
  }
  if (itemId.startsWith("folder:")) {
    const folder = index.folderById.get(itemId.slice("folder:".length));
    return folder
      ? {
          collectionId: folder.collectionId,
          parentFolderId: folder.id,
        }
      : null;
  }
  return null;
}

function sameLocation(
  left: DropLocation | ApiSavedRequest,
  right: DropLocation,
) {
  return (
    left.collectionId === right.collectionId &&
    left.parentFolderId === right.parentFolderId
  );
}

function sameOrder(left: string[], right: string[]) {
  return left.length === right.length && left.every((id, index) => id === right[index]);
}

function locationKey(location: DropLocation) {
  return `${location.collectionId}:${location.parentFolderId ?? ""}`;
}
