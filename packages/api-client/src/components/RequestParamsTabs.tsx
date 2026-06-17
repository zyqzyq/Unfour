import type * as React from "react";
import Editor from "@monaco-editor/react";
import { Info, Plus, Save } from "lucide-react";
import { Button, Input, cn } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import {
  duplicateEnvironmentKeys,
  isSensitiveKey,
} from "../request-utils";
import { requestConfigTabs } from "../model/request-tabs";
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
        items={requestConfigTabs.map((item) => ({
          ...item,
          meta:
            item.id === "query"
              ? enabledCount(query)
              : item.id === "auth"
                ? envVariables.length
                : item.id === "headers"
                  ? enabledCount(headers)
                  : item.id === "body" && body.trim()
                    ? 1
                    : 0,
        }))}
        onChange={onTabChange}
      />
      <div className="min-h-0 flex-1 overflow-hidden">
        {tab === "query" && (
          <PaneScroll>
            <p className="mb-2 text-[11px] text-[var(--u-color-text-muted)]">
              Query parameters appended to the request URL.
            </p>
            <KeyValueEditor
              items={query}
              onChange={onQueryChange}
              showTitle={false}
              title="Query params"
            />
          </PaneScroll>
        )}
        {tab === "headers" && (
          <PaneScroll>
            <p className="mb-2 text-[11px] text-[var(--u-color-text-muted)]">
              Request headers sent with this call.
            </p>
            <KeyValueEditor
              items={headers}
              onChange={onHeadersChange}
              showTitle={false}
              title="Headers"
            />
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
              theme="vs-dark"
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
        {tab === "settings" && (
          <PaneScroll>
            <SettingsPanel />
          </PaneScroll>
        )}
      </div>
    </>
  );
}

export function CompactTabs<T extends string>({
  active,
  className,
  items,
  onChange,
}: {
  active: T;
  className?: string;
  items: Array<{ id: T; label: string; meta?: number }>;
  onChange: (tab: T) => void;
}) {
  return (
    <div
      className={cn(
        "flex h-[var(--u-size-tabbar)] shrink-0 items-end gap-4 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3",
        className,
      )}
    >
      {items.map((item) => (
        <button
          className={cn(
            "relative flex h-full min-w-0 items-center gap-1.5 px-1 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors after:absolute after:inset-x-0 after:bottom-[-1px] after:h-0.5 after:bg-transparent",
            active === item.id
              ? "text-[var(--u-color-text)] after:bg-[var(--u-color-primary)]"
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
  showTitle = true,
  title,
}: {
  items: KeyValue[];
  maskSensitiveValues?: boolean;
  onChange: (items: KeyValue[]) => void;
  showTitle?: boolean;
  title: string;
}) {
  function update(index: number, patch: Partial<KeyValue>) {
    onChange(items.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)));
  }

  const cellInputClass =
    "h-[32px] rounded-none border-0 bg-transparent px-0 text-[12px] hover:border-0 focus:border-0 focus:ring-0 disabled:bg-transparent disabled:text-[var(--u-color-text-soft)]";

  return (
    <div className="space-y-1.5">
      <div className={cn("flex items-center", showTitle ? "justify-between" : "justify-end")}>
        {showTitle && (
          <span className="text-xs font-semibold uppercase text-[var(--u-color-text-muted)]">
            {title}
          </span>
        )}
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
      <div className="overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
        <div className="grid min-h-[28px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_minmax(140px,1.1fr)] items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <span />
          <span>Key</span>
          <span>Value</span>
          <span>Description</span>
        </div>
        {(items.length ? items : [{ key: "", value: "", enabled: false }]).map((item, index) => (
          <div
            className="grid min-h-[34px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_minmax(140px,1.1fr)] items-center gap-2 border-b border-[var(--u-color-border)] px-2 last:border-b-0"
            key={`${title}-${index}`}
          >
            <input
              checked={item.enabled}
              className="h-4 w-4"
              disabled={!items.length}
              onChange={(event) => update(index, { enabled: event.target.checked })}
              type="checkbox"
            />
            <Input
              className={cellInputClass}
              disabled={!items.length}
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder="Key"
              value={item.key}
            />
            <Input
              className={cellInputClass}
              disabled={!items.length}
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder="Value"
              type={maskSensitiveValues && isSensitiveKey(item.key) ? "password" : "text"}
              value={item.value}
            />
            <Input
              className={cellInputClass}
              disabled
              placeholder="Description"
              value=""
            />
          </div>
        ))}
      </div>
    </div>
  );
}

function SettingsPanel() {
  return (
    <div className="max-w-xl overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
      {[
        ["Follow redirects", "Handled by the current request execution defaults."],
        ["Verify TLS certificates", "Controlled by the backend execution boundary."],
        ["Request timeout", "Uses the existing Command Bus request timeout behavior."],
      ].map(([label, description]) => (
        <div
          className="flex min-h-[52px] items-center justify-between gap-4 border-b border-[var(--u-color-border)] p-3 last:border-b-0"
          key={label}
        >
          <div className="min-w-0">
            <div className="text-[13px] font-medium text-[var(--u-color-text)]">
              {label}
            </div>
            <div className="text-[12px] text-[var(--u-color-text-muted)]">
              {description}
            </div>
          </div>
          <span className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 py-1 text-[11px] text-[var(--u-color-text-soft)]">
            Current
          </span>
        </div>
      ))}
      <div className="flex items-start gap-2 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 text-[12px] text-[var(--u-color-text-muted)]">
        <Info className="mt-0.5 shrink-0 text-[var(--u-color-primary)]" size={14} />
        <span>
          Settings are shown for layout parity. Changing request execution policy
          requires a separate backend contract update.
        </span>
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
        <div className="rounded-md bg-[var(--u-color-warning-soft)] px-2 py-1 text-[var(--u-color-warning-text)] ring-1 ring-inset ring-[var(--u-badge-warning-ring)]">
          Duplicate variables: {duplicateKeys.join(", ")}
        </div>
      )}
      {sensitiveKeys.length > 0 && (
        <div className="rounded-md bg-[var(--u-badge-neutral-bg)] px-2 py-1 text-[var(--u-color-text-muted)] ring-1 ring-inset ring-[var(--u-badge-neutral-ring)]">
          Sensitive-looking values are masked locally: {sensitiveKeys.join(", ")}
        </div>
      )}
    </div>
  );
}

function enabledCount(items: KeyValue[]) {
  return items.filter((item) => item.enabled).length;
}
