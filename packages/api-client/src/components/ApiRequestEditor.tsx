import type { KeyValue } from "@unfour/command-client";
import type {
  ApiAuthConfig,
  RequestBodyMode,
  RequestParamsTab,
  RequestRawBodyType,
} from "../model/types";
import { RequestParamsTabs } from "./RequestParamsTabs";

export function ApiRequestEditor({
  auth,
  body,
  bodyMode,
  formBody,
  headers,
  onAuthChange,
  onBodyChange,
  onBodyModeChange,
  onFormBodyChange,
  onHeadersChange,
  onQueryChange,
  onRawBodyTypeChange,
  onTabChange,
  query,
  rawBodyType,
  tab,
}: {
  auth: ApiAuthConfig;
  body: string;
  bodyMode: RequestBodyMode;
  formBody: KeyValue[];
  headers: KeyValue[];
  onAuthChange: (value: ApiAuthConfig) => void;
  onBodyChange: (value: string) => void;
  onBodyModeChange: (value: RequestBodyMode) => void;
  onFormBodyChange: (items: KeyValue[]) => void;
  onHeadersChange: (items: KeyValue[]) => void;
  onQueryChange: (items: KeyValue[]) => void;
  onRawBodyTypeChange: (value: RequestRawBodyType) => void;
  onTabChange: (tab: RequestParamsTab) => void;
  query: KeyValue[];
  rawBodyType: RequestRawBodyType;
  tab: RequestParamsTab;
}) {
  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col">
      <RequestParamsTabs
        auth={auth}
        body={body}
        bodyMode={bodyMode}
        formBody={formBody}
        headers={headers}
        onAuthChange={onAuthChange}
        onBodyChange={onBodyChange}
        onBodyModeChange={onBodyModeChange}
        onFormBodyChange={onFormBodyChange}
        onHeadersChange={onHeadersChange}
        onQueryChange={onQueryChange}
        onRawBodyTypeChange={onRawBodyTypeChange}
        onTabChange={onTabChange}
        query={query}
        rawBodyType={rawBodyType}
        tab={tab}
      />
    </section>
  );
}
