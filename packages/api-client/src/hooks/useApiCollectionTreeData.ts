import { useQuery } from "@tanstack/react-query";
import {
  listApiHistory,
  listSavedApiRequests,
  type ApiCollectionFolder,
} from "@unfour/command-client";

export function useApiCollectionTreeData(
  workspaceId: string,
  search: string,
  folders: ApiCollectionFolder[],
) {
  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });
  const requests = savedQuery.data ?? [];
  const folderNameById = new Map(folders.map((folder) => [folder.id, folder.name]));
  const searchText = search.trim().toLowerCase();
  const visibleRequests = requests.filter((request) =>
    searchText
      ? [
          request.name,
          request.url,
          request.method,
          request.parentFolderId
            ? (folderNameById.get(request.parentFolderId) ?? "")
            : "",
        ].some((value) => value.toLowerCase().includes(searchText))
      : true,
  );
  const historyItems = (historyQuery.data ?? []).filter((item) =>
    searchText
      ? [item.name ?? "", item.url, item.method, String(item.status ?? "")]
          .some((value) => value.toLowerCase().includes(searchText))
      : true,
  );

  // Keep the full list for name validation even when search hides a conflict.
  return { requests, visibleRequests, historyItems };
}
