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
  onPostResponseScriptChange,
  onPreRequestScriptChange,
  onQueryChange,
  onRawBodyTypeChange,
  onTabChange,
  onTimeoutChange,
  query,
  rawBodyType,
  postResponseScript,
  preRequestScript,
  tab,
  timeoutMs,
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
  onPostResponseScriptChange: (value: string) => void;
  onPreRequestScriptChange: (value: string) => void;
  onQueryChange: (items: KeyValue[]) => void;
  onRawBodyTypeChange: (value: RequestRawBodyType) => void;
  onTabChange: (tab: RequestParamsTab) => void;
  onTimeoutChange: (value: number | null) => void;
  query: KeyValue[];
  rawBodyType: RequestRawBodyType;
  postResponseScript: string;
  preRequestScript: string;
  tab: RequestParamsTab;
  timeoutMs: number | null;
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
        onPostResponseScriptChange={onPostResponseScriptChange}
        onPreRequestScriptChange={onPreRequestScriptChange}
        onQueryChange={onQueryChange}
        onRawBodyTypeChange={onRawBodyTypeChange}
        onTabChange={onTabChange}
        onTimeoutChange={onTimeoutChange}
        query={query}
        rawBodyType={rawBodyType}
        postResponseScript={postResponseScript}
        preRequestScript={preRequestScript}
        tab={tab}
        timeoutMs={timeoutMs}
      />
    </section>
  );
}
