import { useState } from "react";
import type * as React from "react";
import Editor from "@monaco-editor/react";
import { Info, Plus, Save, Trash2, Wand2 } from "lucide-react";
import { Button, Input, cn } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import {
  duplicateEnvironmentKeys,
  isSensitiveKey,
} from "../request-utils";
import { requestConfigTabs } from "../model/request-tabs";
import type {
  ApiAuthConfig,
  ApiAuthPlacement,
  RequestBodyMode,
  RequestParamsTab,
  RequestRawBodyType,
} from "../model/types";

export function RequestParamsTabs({
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
    <>
      <CompactTabs
        active={tab}
        items={requestConfigTabs.map((item) => ({
          ...item,
          meta:
            item.id === "query"
              ? enabledCount(query)
              : item.id === "auth"
                ? auth.type === "none"
                  ? 0
                  : 1
                : item.id === "headers"
                  ? enabledCount(headers)
                  : item.id === "body"
                    ? bodyMode === "none"
                      ? 0
                      : bodyMode === "form"
                        ? enabledCount(formBody)
                        : body.trim()
                          ? 1
                          : 0
                    : 0,
        }))}
        onChange={onTabChange}
      />
      <div className="min-h-0 flex-1 overflow-hidden">
        {tab === "query" && (
          <PaneScroll>
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
            <KeyValueEditor
              items={headers}
              onChange={onHeadersChange}
              showTitle={false}
              title="Headers"
            />
          </PaneScroll>
        )}
        {tab === "body" && (
          <BodyEditor
            body={body}
            bodyMode={bodyMode}
            formBody={formBody}
            onBodyChange={onBodyChange}
            onBodyModeChange={onBodyModeChange}
            onFormBodyChange={onFormBodyChange}
            onRawBodyTypeChange={onRawBodyTypeChange}
            rawBodyType={rawBodyType}
          />
        )}
        {tab === "auth" && (
          <PaneScroll>
            <AuthPanel auth={auth} onAuthChange={onAuthChange} />
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
        "flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-1 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3",
        className,
      )}
    >
      {items.map((item) => (
        <button
          className={cn(
            "relative flex h-full min-w-0 items-center gap-1.5 px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors after:absolute after:inset-x-0 after:bottom-[-1px] after:h-0.5 after:bg-transparent",
            active === item.id
              ? "text-[var(--u-color-text)] after:bg-[var(--u-color-primary)]"
              : "hover:text-[var(--u-color-text)]",
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
    if (index === items.length) {
      onChange([...items, { enabled: true, key: "", value: "", ...patch }]);
      return;
    }
    onChange(items.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)));
  }

  function remove(index: number) {
    onChange(items.filter((_, itemIndex) => itemIndex !== index));
  }

  const cellInputClass =
    "h-[32px] rounded-none border-0 bg-transparent px-0 text-[12px] hover:border-0 focus:border-0 focus:ring-0 disabled:bg-transparent disabled:text-[var(--u-color-text-soft)]";
  const rows = [...items, { key: "", value: "", enabled: true }];

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
        <div className="grid min-h-[28px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <span />
          <span>Key</span>
          <span>Value</span>
          <span />
        </div>
        {rows.map((item, index) => (
          <div
            className="grid min-h-[34px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center gap-2 border-b border-[var(--u-color-border)] px-2 last:border-b-0"
            key={`${title}-${index}`}
          >
            <input
              checked={item.enabled}
              className="h-4 w-4"
              onChange={(event) => update(index, { enabled: event.target.checked })}
              type="checkbox"
            />
            <Input
              className={cellInputClass}
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder="Key"
              value={item.key}
            />
            <Input
              className={cellInputClass}
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder="Value"
              type={maskSensitiveValues && isSensitiveKey(item.key) ? "password" : "text"}
              value={item.value}
            />
            <button
              aria-label={`Delete ${title} row`}
              className="grid h-7 w-7 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)] disabled:pointer-events-none disabled:opacity-0"
              disabled={index === items.length}
              onClick={() => remove(index)}
              type="button"
            >
              <Trash2 size={13} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

function BodyEditor({
  body,
  bodyMode,
  formBody,
  onBodyChange,
  onBodyModeChange,
  onFormBodyChange,
  onRawBodyTypeChange,
  rawBodyType,
}: {
  body: string;
  bodyMode: RequestBodyMode;
  formBody: KeyValue[];
  onBodyChange: (value: string) => void;
  onBodyModeChange: (value: RequestBodyMode) => void;
  onFormBodyChange: (items: KeyValue[]) => void;
  onRawBodyTypeChange: (value: RequestRawBodyType) => void;
  rawBodyType: RequestRawBodyType;
}) {
  const [formatError, setFormatError] = useState<string | null>(null);
  const jsonError =
    bodyMode === "raw" && rawBodyType === "json" && body.trim()
      ? getJsonError(body)
      : null;

  function formatJson() {
    try {
      onBodyChange(JSON.stringify(JSON.parse(body), null, 2));
      setFormatError(null);
    } catch (error) {
      setFormatError(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex min-h-[38px] shrink-0 items-center justify-between gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3">
        <SegmentedControl<RequestBodyMode>
          items={[
            { id: "none", label: "none" },
            { id: "raw", label: "raw" },
            { id: "form", label: "x-www-form-urlencoded" },
          ]}
          onChange={onBodyModeChange}
          value={bodyMode}
        />
        {bodyMode === "raw" && (
          <div className="flex items-center gap-2">
            <select
              aria-label="Raw body type"
              className="h-[var(--u-size-button-compact)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-2 text-[12px] text-[var(--u-color-text)] outline-none"
              onChange={(event) =>
                onRawBodyTypeChange(event.target.value as RequestRawBodyType)
              }
              value={rawBodyType}
            >
              <option value="json">JSON</option>
              <option value="text">Text</option>
            </select>
            {rawBodyType === "json" && (
              <Button onClick={formatJson} size="sm" type="button" variant="outline">
                <Wand2 size={13} />
                Format JSON
              </Button>
            )}
          </div>
        )}
      </div>
      {(jsonError || formatError) && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] px-3 py-1 text-[12px] text-[var(--u-color-danger)]">
          JSON error: {formatError ?? jsonError}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-hidden">
        {bodyMode === "none" && (
          <div className="flex h-full items-center justify-center text-[13px] text-[var(--u-color-text-muted)]">
            No request body
          </div>
        )}
        {bodyMode === "raw" && (
          <Editor
            defaultLanguage={rawBodyType === "json" ? "json" : "plaintext"}
            onChange={(value) => {
              setFormatError(null);
              onBodyChange(value ?? "");
            }}
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
        )}
        {bodyMode === "form" && (
          <PaneScroll>
            <KeyValueEditor
              items={formBody}
              onChange={onFormBodyChange}
              showTitle={false}
              title="Form fields"
            />
          </PaneScroll>
        )}
      </div>
    </div>
  );
}

function AuthPanel({
  auth,
  onAuthChange,
}: {
  auth: ApiAuthConfig;
  onAuthChange: (value: ApiAuthConfig) => void;
}) {
  const inputClass =
    "h-[var(--u-size-input)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-2 text-[12px] text-[var(--u-color-text)] outline-none focus:border-[var(--u-color-focus)]";
  return (
    <div className="mb-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[12px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          Auth
        </span>
        <select
          aria-label="Auth type"
          className={inputClass}
          onChange={(event) => onAuthTypeChange(event.target.value, onAuthChange)}
          value={auth.type}
        >
          <option value="none">No Auth</option>
          <option value="bearer">Bearer Token</option>
          <option value="basic">Basic Auth</option>
          <option value="api-key">API Key</option>
        </select>
      </div>
      {auth.type === "none" && (
        <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-2 text-[12px] text-[var(--u-color-text-muted)]">
          No auth applied
        </div>
      )}
      {auth.type === "bearer" && (
        <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
          Token
          <Input
            onChange={(event) =>
              onAuthChange({ ...auth, token: event.target.value })
            }
            placeholder="{{token}}"
            type="password"
            value={auth.token}
          />
        </label>
      )}
      {auth.type === "basic" && (
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            Username
            <Input
              onChange={(event) =>
                onAuthChange({ ...auth, username: event.target.value })
              }
              value={auth.username}
            />
          </label>
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            Password
            <Input
              onChange={(event) =>
                onAuthChange({ ...auth, password: event.target.value })
              }
              type="password"
              value={auth.password}
            />
          </label>
        </div>
      )}
      {auth.type === "api-key" && (
        <div className="grid gap-2 sm:grid-cols-[minmax(120px,1fr)_minmax(120px,1fr)_120px]">
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            Key
            <Input
              onChange={(event) =>
                onAuthChange({ ...auth, key: event.target.value })
              }
              placeholder="x-api-key"
              value={auth.key}
            />
          </label>
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            Value
            <Input
              onChange={(event) =>
                onAuthChange({ ...auth, value: event.target.value })
              }
              type="password"
              value={auth.value}
            />
          </label>
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            Add to
            <select
              aria-label="API key placement"
              className={inputClass}
              onChange={(event) =>
                onAuthChange({
                  ...auth,
                  addTo: event.target.value as ApiAuthPlacement,
                })
              }
              value={auth.addTo}
            >
              <option value="header">Header</option>
              <option value="query">Query Param</option>
            </select>
          </label>
        </div>
      )}
    </div>
  );
}

function SegmentedControl<T extends string>({
  items,
  onChange,
  value,
}: {
  items: Array<{ id: T; label: string }>;
  onChange: (value: T) => void;
  value: T;
}) {
  return (
    <div className="flex overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
      {items.map((item) => (
        <button
          aria-pressed={value === item.id}
          className={cn(
            "h-[var(--u-size-button-compact)] border-r border-[var(--u-color-border)] px-2 text-[12px] last:border-r-0",
            value === item.id
              ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
              : "bg-[var(--u-color-bg)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
          )}
          key={item.id}
          onClick={() => onChange(item.id)}
          type="button"
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

function onAuthTypeChange(
  value: string,
  onAuthChange: (value: ApiAuthConfig) => void,
) {
  if (value === "bearer") {
    onAuthChange({ type: "bearer", token: "" });
    return;
  }
  if (value === "basic") {
    onAuthChange({ type: "basic", username: "", password: "" });
    return;
  }
  if (value === "api-key") {
    onAuthChange({ type: "api-key", addTo: "header", key: "", value: "" });
    return;
  }
  onAuthChange({ type: "none" });
}

function getJsonError(value: string): string | null {
  try {
    JSON.parse(value);
    return null;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
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
