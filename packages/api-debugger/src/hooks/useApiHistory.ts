import { useMutation, useQuery } from "@tanstack/react-query";
import {
  getApiHistoryDetail,
  listApiHistory,
  type ApiHistoryDetail,
} from "@unfour/command-client";

export function useApiHistory({
  onReplayLoaded,
  workspaceId,
}: {
  onReplayLoaded: (history: ApiHistoryDetail) => void;
  workspaceId: string;
}) {
  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });

  const replayHistoryMutation = useMutation({
    mutationFn: (historyId: string) => getApiHistoryDetail(workspaceId, historyId),
    onSuccess: onReplayLoaded,
  });

  return {
    historyQuery,
    replayHistoryMutation,
  };
}
