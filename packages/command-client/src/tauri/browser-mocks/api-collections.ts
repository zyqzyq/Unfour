import * as collections from "./api-collection-lifecycle";
import * as folders from "./api-collection-folders";
import * as requests from "./api-request-organization";
import { UNHANDLED, type MockResult } from "./types";

// Registration is independent of result: folder deletion returns handled undefined.
const handlers = new Map<string, (args?: Record<string, unknown>) => unknown>([
  ["api_collection_list", collections.listMockCollections],
  ["api_collection_export", collections.exportMockCollection],
  ["api_collection_import", collections.importMockCollection],
  ["api_collection_create", collections.createMockCollection],
  ["api_collection_rename", collections.renameMockCollection],
  ["api_collection_delete", collections.deleteMockCollection],
  ["api_collection_folders_list", folders.listMockCollectionFolders],
  ["api_collection_folder_create", folders.createMockCollectionFolder],
  ["api_collection_folder_rename", folders.renameMockCollectionFolder],
  ["api_collection_folder_delete", folders.deleteMockCollectionFolder],
  ["api_collection_folder_move", folders.moveMockCollectionFolder],
  ["api_collection_folders_reorder", folders.reorderMockCollectionFolders],
  ["api_request_move", requests.moveMockRequest],
  ["api_requests_reorder", requests.reorderMockRequests],
  ["api_collection_import_preview", () => null],
]);

export function handleApiCollectionMock<T>(command: string, args?: Record<string, unknown>): MockResult<T> {
  const handler = handlers.get(command);
  return handler ? handler(args) as T : UNHANDLED;
}
