import type { ApiCollectionFolder } from "../../types";

type CollectionScope = Pick<ApiCollectionFolder, "workspaceId" | "collectionId" | "parentFolderId">;
type OrderedRow = CollectionScope & Pick<ApiCollectionFolder, "id" | "sortOrder" | "updatedAt">;

/** Apply the supplied order sequentially, preserving partial and duplicate lists. */
export function reorderMockCollectionRows(
  rows: OrderedRow[],
  ids: string[],
  scope: CollectionScope,
  notFoundMessage: string,
): void {
  ids.forEach((id, index) => {
    const row = rows.find((item) =>
      item.workspaceId === scope.workspaceId &&
      item.collectionId === scope.collectionId &&
      item.parentFolderId === scope.parentFolderId &&
      item.id === id,
    );
    if (!row) throw new Error(notFoundMessage);
    row.sortOrder = index;
    row.updatedAt = new Date().toISOString();
  });
}
