import type { ApiHistoryItem, ApiResponse } from "@unfour/command-client";
import type { ResponsePanelTab, ResponseTab } from "../model/types";
import { ResponseTabs } from "./ResponseTabs";

export function ApiResponseViewer({
  historyItems,
  loadingReplay,
  onReplay,
  onResponseTabChange,
  onResultTabChange,
  response,
  responseTab,
  resultTab,
  sending,
}: {
  historyItems: ApiHistoryItem[];
  loadingReplay: boolean;
  onReplay: (item: ApiHistoryItem) => void;
  onResponseTabChange: (tab: ResponseTab) => void;
  onResultTabChange: (tab: ResponsePanelTab) => void;
  response: ApiResponse | null;
  responseTab: ResponseTab;
  resultTab: ResponsePanelTab;
  sending: boolean;
}) {
  return (
    <ResponseTabs
      historyItems={historyItems}
      loadingReplay={loadingReplay}
      onReplay={onReplay}
      onResponseTabChange={onResponseTabChange}
      onResultTabChange={onResultTabChange}
      response={response}
      responseTab={responseTab}
      resultTab={resultTab}
      sending={sending}
    />
  );
}
