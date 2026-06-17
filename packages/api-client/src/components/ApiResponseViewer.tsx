import type { ApiRequestTab } from "../model/request-tabs";
import type { ResponseTab } from "../model/types";
import { ResponseTabs } from "./ResponseTabs";

export function ApiResponseViewer({
  onResponseTabChange,
  tab,
}: {
  onResponseTabChange: (tab: ResponseTab) => void;
  tab: ApiRequestTab;
}) {
  return (
    <ResponseTabs
      onResponseTabChange={onResponseTabChange}
      tab={tab}
    />
  );
}
