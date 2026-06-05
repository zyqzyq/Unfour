import type * as React from "react";
import Editor from "@monaco-editor/react";
import { Plus, Save } from "lucide-react";
import { Button, Input, cn } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import {
  duplicateEnvironmentKeys,
  isSensitiveKey,
} from "../request-utils";
import type { RequestParamsTab } from "../model/types";

export function RequestParamsTabs({
  body,
  envVariables,
  headers,
  onBodyChange,
  onEnvVariablesChange,
  onHeadersChange,
  onQueryChange,
  onSaveEnvironment,
  onTabChange,
  query,
  savingEnvironment,
  tab,
}: {
  body: string;
  envVariables: KeyValue[];
  headers: KeyValue[];
  onBodyChange: (value: string) => void;
  onEnvVariablesChange: (items: KeyValue[]) => void;
  onHeadersChange: (items: KeyValue[]) => void;
  onQueryChange: (items: KeyValue[]) => void;
  onSaveEnvironment: () => void;
  onTabChange: (tab: RequestParamsTab) => void;
  query: KeyValue[];
  savingEnvironment: boolean;
  tab: RequestParamsTab;
}) {
  return (
    <>
      <CompactTabs
        active={tab}
        items={[
          { id: "query", label: "Query", meta: enabledCount(query) },
          { id: "headers", label: "Headers", meta: enabledCount(headers) },
          { id: "body", label: "Body", meta: body.trim() ? 1 : 0 },
          { id: "auth", label: "Auth", meta: envVariables.length },
        ]}
        onChange={onTabChange}
      />
      <div className="min-h-0 flex-1 overflow-hidden">
        {tab === "query" && (
          <PaneScroll>
            <KeyValueEditor items={query} onChange={onQueryChange} title="Query params" />
          </PaneScroll>
        )}
        {tab === "headers" && (
          <PaneScroll>
            <KeyValueEditor items={headers} onChange={onHeadersChange} title="Headers" />
          </PaneScroll>
        )}
        {tab === "body" && (
          <div className="h-full min-h-0">
            <Editor
              defaultLanguage="json"
              onChange={(value) => onBodyChange(value ?? "")}
              options={{
                fontSize: 13,
                lineNumbersMinChars: 3,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                wordWrap: "on",
              }}
              theme="vs-light"
              value={body}
            />
          </div>
        )}
        {tab === "auth" && (
          <PaneScroll>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-[12px] font-semibold uppercase text-[var(--u-color-text-soft)]">
                Environment variables
              </span>
              <Button
                disabled={savingEnvironment}
                onClick={onSaveEnvironment}
                size="sm"
                type="button"
                variant="outline"
              >
                <Save size={13} />
                Save
              </Button>
            </div>
            <KeyValueEditor
              items={envVariables}
              maskSensitiveValues
              onChange={onEnvVariablesChange}
              title="Variables"
            />
            <EnvironmentHints variables={envVariables} />
          </PaneScroll>
        )}
      </div>
    </>
  );
}

export function CompactTabs<T extends string>({
  active,
  items,
  onChange,
}: {
  active: T;
  items: Array<{ id: T; label: string; meta?: number }>;
  onChange: (tab: T) => void;
}) {
  return (
    <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-end gap-1 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
      {items.map((item) => (
        <button
          className={cn(
            "flex h-[29px] min-w-0 items-center gap-1.5 rounded-t-[var(--u-radius-sm)] border border-transparent px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors",
            active === item.id
              ? "border-[var(--u-color-border)] border-b-[var(--u-color-surface)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
              : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
          )}
          key={item.id}
          onClick={() => onChange(item.id)}
          type="button"
        >
          <span className="truncate">{item.label}</span>
          {typeof item.meta === "number" && item.meta > 0 && (
            <span className="rounded-[var(--u-radius-sm)] bg-[var(--u-color-surface-muted)] px-1 text-[10px] leading-4 text-[var(--u-color-text-soft)]">
              {item.meta}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

function PaneScroll({ children }: { children: React.ReactNode }) {
  return <div className="h-full min-h-0 overflow-auto p-3">{children}</div>;
}

function KeyValueEditor({
  items,
  maskSensitiveValues = false,
  onChange,
  title,
}: {
  items: KeyValue[];
  maskSensitiveValues?: boolean;
  onChange: (items: KeyValue[]) => void;
  title: string;
}) {
  function update(index: number, patch: Partial<KeyValue>) {
    onChange(items.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)));
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
          {title}
        </span>
        <Button
          onClick={() => onChange([...items, { key: "", value: "", enabled: true }])}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus size={13} />
          Add
        </Button>
      </div>
      <div className="space-y-2">
        {items.map((item, index) => (
          <div className="grid grid-cols-[20px_1fr_1fr] gap-2" key={`${title}-${index}`}>
            <input
              checked={item.enabled}
              className="mt-2 h-4 w-4"
              onChange={(event) => update(index, { enabled: event.target.checked })}
              type="checkbox"
            />
            <Input
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder="Key"
              value={item.key}
            />
            <Input
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder="Value"
              type={maskSensitiveValues && isSensitiveKey(item.key) ? "password" : "text"}
              value={item.value}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

function EnvironmentHints({ variables }: { variables: KeyValue[] }) {
  const duplicateKeys = duplicateEnvironmentKeys(variables);
  const sensitiveKeys = variables
    .filter((item) => item.enabled && isSensitiveKey(item.key) && item.value.trim())
    .map((item) => item.key.trim());

  if (!duplicateKeys.length && !sensitiveKeys.length) {
    return null;
  }

  return (
    <div className="mt-2 space-y-1 text-xs">
      {duplicateKeys.length > 0 && (
        <div className="rounded-md bg-amber-50 px-2 py-1 text-amber-800 ring-1 ring-inset ring-amber-200">
          Duplicate variables: {duplicateKeys.join(", ")}
        </div>
      )}
      {sensitiveKeys.length > 0 && (
        <div className="rounded-md bg-slate-50 px-2 py-1 text-slate-600 ring-1 ring-inset ring-slate-200">
          Sensitive-looking values are masked locally: {sensitiveKeys.join(", ")}
        </div>
      )}
    </div>
  );
}

function enabledCount(items: KeyValue[]) {
  return items.filter((item) => item.enabled).length;
}
