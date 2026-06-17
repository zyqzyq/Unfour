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
  envVariables,
  formBody,
  headers,
  onAuthChange,
  onBodyChange,
  onBodyModeChange,
  onEnvVariablesChange,
  onFormBodyChange,
  onHeadersChange,
  onQueryChange,
  onRawBodyTypeChange,
  onSaveEnvironment,
  onTabChange,
  query,
  rawBodyType,
  savingEnvironment,
  tab,
}: {
  auth: ApiAuthConfig;
  body: string;
  bodyMode: RequestBodyMode;
  envVariables: KeyValue[];
  formBody: KeyValue[];
  headers: KeyValue[];
  onAuthChange: (value: ApiAuthConfig) => void;
  onBodyChange: (value: string) => void;
  onBodyModeChange: (value: RequestBodyMode) => void;
  onEnvVariablesChange: (items: KeyValue[]) => void;
  onFormBodyChange: (items: KeyValue[]) => void;
  onHeadersChange: (items: KeyValue[]) => void;
  onQueryChange: (items: KeyValue[]) => void;
  onRawBodyTypeChange: (value: RequestRawBodyType) => void;
  onSaveEnvironment: () => void;
  onTabChange: (tab: RequestParamsTab) => void;
  query: KeyValue[];
  rawBodyType: RequestRawBodyType;
  savingEnvironment: boolean;
  tab: RequestParamsTab;
}) {
  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col">
      <RequestParamsTabs
        auth={auth}
        body={body}
        bodyMode={bodyMode}
        envVariables={envVariables}
        formBody={formBody}
        headers={headers}
        onAuthChange={onAuthChange}
        onBodyChange={onBodyChange}
        onBodyModeChange={onBodyModeChange}
        onEnvVariablesChange={onEnvVariablesChange}
        onFormBodyChange={onFormBodyChange}
        onHeadersChange={onHeadersChange}
        onQueryChange={onQueryChange}
        onRawBodyTypeChange={onRawBodyTypeChange}
        onSaveEnvironment={onSaveEnvironment}
        onTabChange={onTabChange}
        query={query}
        rawBodyType={rawBodyType}
        savingEnvironment={savingEnvironment}
        tab={tab}
      />
    </section>
  );
}
