import type * as React from "react";
import { Input } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import { methods } from "../hooks/useApiRequest";
import type { RequestParamsTab } from "../model/types";
import { RequestParamsTabs } from "./RequestParamsTabs";

export function ApiRequestEditor({
  body,
  envVariables,
  folderPath,
  headers,
  method,
  name,
  onBodyChange,
  onEnvVariablesChange,
  onFolderPathChange,
  onHeadersChange,
  onMethodChange,
  onNameChange,
  onQueryChange,
  onSaveEnvironment,
  onTabChange,
  onUrlChange,
  query,
  savingEnvironment,
  tab,
  url,
}: {
  body: string;
  envVariables: KeyValue[];
  folderPath: string;
  headers: KeyValue[];
  method: string;
  name: string;
  onBodyChange: (value: string) => void;
  onEnvVariablesChange: (items: KeyValue[]) => void;
  onFolderPathChange: (value: string) => void;
  onHeadersChange: (items: KeyValue[]) => void;
  onMethodChange: (value: string) => void;
  onNameChange: (value: string) => void;
  onQueryChange: (items: KeyValue[]) => void;
  onSaveEnvironment: () => void;
  onTabChange: (tab: RequestParamsTab) => void;
  onUrlChange: (value: string) => void;
  query: KeyValue[];
  savingEnvironment: boolean;
  tab: RequestParamsTab;
  url: string;
}) {
  return (
    <section className="flex min-h-[320px] min-w-[340px] flex-[0.58] flex-col border-b border-[var(--u-color-border)] xl:min-h-0 xl:border-b-0 xl:border-r">
      <div className="grid shrink-0 grid-cols-[minmax(120px,0.8fr)_minmax(120px,180px)] gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2">
        <FieldGroup title="Name">
          <Input onChange={(event) => onNameChange(event.target.value)} value={name} />
        </FieldGroup>
        <FieldGroup title="Folder">
          <Input
            onChange={(event) => onFolderPathChange(event.target.value)}
            placeholder="Examples / Auth"
            value={folderPath}
          />
        </FieldGroup>
        <div className="col-span-2 grid grid-cols-[104px_minmax(0,1fr)] gap-2">
          <select
            aria-label="HTTP method"
            className="h-[var(--u-size-input)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] px-2 text-[13px] font-semibold text-[var(--u-color-text)] outline-none transition-colors hover:border-[var(--u-color-border-strong)] focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]"
            onChange={(event) => onMethodChange(event.target.value)}
            value={method}
          >
            {methods.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
          <Input
            onChange={(event) => onUrlChange(event.target.value)}
            placeholder="https://api.example.com/resource"
            value={url}
          />
        </div>
      </div>
      <RequestParamsTabs
        body={body}
        envVariables={envVariables}
        headers={headers}
        onBodyChange={onBodyChange}
        onEnvVariablesChange={onEnvVariablesChange}
        onHeadersChange={onHeadersChange}
        onQueryChange={onQueryChange}
        onSaveEnvironment={onSaveEnvironment}
        onTabChange={onTabChange}
        query={query}
        savingEnvironment={savingEnvironment}
        tab={tab}
      />
    </section>
  );
}

function FieldGroup({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <label className="block space-y-2">
      <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">{title}</span>
      {children}
    </label>
  );
}
