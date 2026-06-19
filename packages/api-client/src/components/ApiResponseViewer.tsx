import type { ApiRequestTab } from "../model/request-tabs";
import type { ResponseTab } from "../model/types";
import { ResponseTabs } from "./ResponseTabs";

export function ApiResponseViewer({
  onOpenAuthSettings,
  onResponseTabChange,
  onRetry,
  tab,
}: {
  onOpenAuthSettings: () => void;
  onResponseTabChange: (tab: ResponseTab) => void;
  onRetry: () => void;
  tab: ApiRequestTab;
}) {
  return (
    <ResponseTabs
      onOpenAuthSettings={onOpenAuthSettings}
      onResponseTabChange={onResponseTabChange}
      onRetry={onRetry}
      tab={tab}
    />
  );
}
