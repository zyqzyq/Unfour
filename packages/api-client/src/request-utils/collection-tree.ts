import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiSavedRequest,
} from "@unfour/command-client";

export type FolderNode = {
  collectionId: string;
  folders: FolderNode[];
  id: string;
  name: string;
  parentFolderId: string | null;
  requests: ApiSavedRequest[];
  sortOrder: number;
};

export type FolderTree = {
  folders: FolderNode[];
  rootRequests: ApiSavedRequest[];
};

export type ApiCollectionGroup = {
  collection: ApiCollection;
  id: string;
  name: string;
  tree: FolderTree;
};

/** Flatten every request in a folder tree (root + all nested folders). */
export function collectTreeRequests(tree: FolderTree): ApiSavedRequest[] {
  const result = [...tree.rootRequests];
  const walk = (folders: FolderNode[]) => {
    for (const folder of folders) {
      result.push(...folder.requests);
      walk(folder.folders);
    }
  };
  walk(tree.folders);
  return result;
}

/**
 * Build the visible API collection tree from persisted folder rows and request
 * parent ids. Empty collections and empty folders remain visible.
 */
export function buildApiCollectionTree(
  collections: ApiCollection[],
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
): ApiCollectionGroup[] {
  const byCollection = new Map<string, ApiSavedRequest[]>();
  const collectionIds = new Set(collections.map((collection) => collection.id));
  for (const request of requests) {
    const key = collectionIds.has(request.collectionId)
      ? request.collectionId
      : collections[0]?.id ?? "";
    byCollection.set(key, [...(byCollection.get(key) ?? []), request]);
  }

  const byCollectionFolders = new Map<string, ApiCollectionFolder[]>();
  for (const folder of folders) {
    if (!collectionIds.has(folder.collectionId)) {
      continue;
    }
    byCollectionFolders.set(folder.collectionId, [
      ...(byCollectionFolders.get(folder.collectionId) ?? []),
      folder,
    ]);
  }

  return [...collections]
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((collection) => ({
      collection,
      id: collection.id,
      name: collection.name,
      tree: buildCollectionFolderTree(
        byCollectionFolders.get(collection.id) ?? [],
        byCollection.get(collection.id) ?? [],
      ),
    }));
}

export function groupRequestsByCollection(
  requests: ApiSavedRequest[],
  collections: ApiCollection[],
): ApiCollectionGroup[] {
  return buildApiCollectionTree(collections, [], requests);
}

function buildCollectionFolderTree(
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
): FolderTree {
  const folderById = new Map(folders.map((folder) => [folder.id, folder]));
  const nodeById = new Map<string, FolderNode>();
  for (const folder of folders) {
    nodeById.set(folder.id, {
      collectionId: folder.collectionId,
      folders: [],
      id: folder.id,
      name: folder.name,
      parentFolderId: folder.parentFolderId,
      requests: [],
      sortOrder: folder.sortOrder,
    });
  }

  const root: FolderTree = { folders: [], rootRequests: [] };
  for (const folder of folders) {
    const node = nodeById.get(folder.id);
    if (!node) continue;
    const parent = folder.parentFolderId
      ? nodeById.get(folder.parentFolderId)
      : null;
    if (
      parent &&
      parent.collectionId === folder.collectionId &&
      !hasAncestor(folderById, folder.parentFolderId, folder.id)
    ) {
      parent.folders.push(node);
    } else {
      root.folders.push(node);
    }
  }

  for (const request of requests) {
    const parent = request.parentFolderId
      ? nodeById.get(request.parentFolderId)
      : null;
    if (parent && parent.collectionId === request.collectionId) {
      parent.requests.push(request);
    } else {
      root.rootRequests.push(request);
    }
  }

  sortTree(root);
  return root;
}

function hasAncestor(
  folderById: Map<string, ApiCollectionFolder>,
  parentFolderId: string | null,
  targetId: string,
) {
  let current = parentFolderId;
  const seen = new Set<string>();
  while (current) {
    if (current === targetId) return true;
    if (seen.has(current)) return true;
    seen.add(current);
    current = folderById.get(current)?.parentFolderId ?? null;
  }
  return false;
}

function sortTree(tree: FolderTree) {
  tree.folders.sort(compareFolderNodes);
  tree.rootRequests.sort(compareRequests);
  for (const folder of tree.folders) {
    sortFolderNode(folder);
  }
}

function sortFolderNode(node: FolderNode) {
  node.folders.sort(compareFolderNodes);
  node.requests.sort(compareRequests);
  for (const child of node.folders) {
    sortFolderNode(child);
  }
}

function compareFolderNodes(left: FolderNode, right: FolderNode) {
  return left.sortOrder - right.sortOrder || left.name.localeCompare(right.name);
}

function compareRequests(left: ApiSavedRequest, right: ApiSavedRequest) {
  return left.sortOrder - right.sortOrder || left.name.localeCompare(right.name);
}
