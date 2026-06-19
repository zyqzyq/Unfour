import { useState, type ReactNode } from "react";
import Editor from "@monaco-editor/react";
import {
  AlertCircle,
  CheckCircle2,
  Copy,
  Loader2,
  Send,
  WifiOff,
} from "lucide-react";
import { Badge, Button, EmptyState, cn, useI18n } from "@unfour/ui";
import type { ApiResponse, KeyValue } from "@unfour/command-client";
import { formatByteSize } from "../request-utils";
import { formatResponseBody, looksLikeJson } from "../model/api-request-state";
import {
  deriveTabResponseState,
  type ApiRequestTab,
} from "../model/request-tabs";
import type { ResponseTab } from "../model/types";
import { CompactTabs } from "./RequestParamsTabs";

export function ResponseTabs({
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
  const { t } = useI18n();
  const responseState = deriveTabResponseState(tab);
  const responseSize = tab.response
    ? formatByteSize(new TextEncoder().encode(tab.response.body).length)
    : null;
  const showResponseTabs = tab.sending || Boolean(tab.response);
  const canRetry = Boolean(tab.draft.url.trim()) && !tab.sending;

  return (
    <section className="relative flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-surface)]">
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3">
        <span className="text-[11px] font-bold uppercase tracking-[0.07em] text-[var(--u-color-text-muted)]">
          {t("api.response.title")}
        </span>
        <ResponseStatus response={tab.response} state={responseState} />
        {tab.response && (
          <div className="ml-auto flex items-center gap-3 font-mono text-[12px] text-[var(--u-color-text-muted)]">
            <span className="font-semibold text-[var(--u-color-text)]">
              {tab.response.durationMs}ms
            </span>
            <span className="font-semibold text-[var(--u-color-text)]">
              {responseSize}
            </span>
            <button
              aria-label={t("api.response.copyBody")}
              className="grid h-7 w-7 place-items-center rounded-[var(--u-radius-md)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]"
              onClick={() => void navigator.clipboard?.writeText(tab.response?.body ?? "")}
              title={t("api.response.copyBody")}
              type="button"
            >
              <Copy size={14} />
            </button>
          </div>
        )}
      </div>
      {showResponseTabs && (
        <CompactTabs
          active={tab.responseTab}
          className="h-[30px] border-b bg-[var(--u-color-surface)]"
          items={[
            { id: "body", label: t("api.response.tabs.body") },
            {
              id: "headers",
              label: t("api.response.tabs.headers"),
              meta: tab.response?.headers.length ?? 0,
            },
            {
              id: "cookies",
              label: t("api.response.tabs.cookies"),
              meta: responseCookies(tab.response).length,
            },
            { id: "timing", label: t("api.response.tabs.timing") },
          ]}
          onChange={onResponseTabChange}
        />
      )}
      <div className="min-h-0 flex-1 overflow-hidden">
        {responseState === "sending" && <SendingState />}
        {responseState === "idle" && (
          <ResponsePaneState
            description={t("api.response.emptyDescription")}
            icon={<Send size={24} />}
            title={t("api.response.emptyTitle")}
            tone="empty"
          >
            <Button
              disabled={!canRetry}
              onClick={onRetry}
              size="sm"
              type="button"
              variant="outline"
            >
              <Send size={13} />
              {t("api.actions.send")}
              <span className="ml-1 flex items-center gap-0.5">
                <kbd className="rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] px-1 text-[10px]">
                  Ctrl
                </kbd>
                <kbd className="rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] px-1 text-[10px]">
                  Enter
                </kbd>
              </span>
            </Button>
          </ResponsePaneState>
        )}
        {(responseState === "network" ||
          responseState === "timeout" ||
          responseState === "failed") && (
          <ResponsePaneState
            description={
              tab.sendError ||
              (responseState === "timeout"
                ? t("api.response.timeoutDescription")
                : t("api.response.errorDescription"))
            }
            icon={responseState === "network" ? <WifiOff size={24} /> : <AlertCircle size={24} />}
            title={
              responseState === "network"
                ? t("api.response.networkTitle")
                : responseState === "timeout"
                  ? t("api.response.timeoutTitle")
                  : t("api.response.failedTitle")
            }
            tone="error"
          >
            <Button
              disabled={!canRetry}
              onClick={onRetry}
              size="sm"
              type="button"
              variant="outline"
            >
              {t("api.response.retry")}
            </Button>
          </ResponsePaneState>
        )}
        {responseState !== "sending" &&
          !tab.sendError &&
          tab.responseTab === "body" && (
            <ResponseBodyView
              onOpenAuthSettings={onOpenAuthSettings}
              response={tab.response}
            />
          )}
        {!tab.sendError && tab.responseTab === "headers" && (
          <KeyValueReadout
            emptyLabel={t("api.response.noHeaders")}
            items={tab.response?.headers ?? []}
          />
        )}
        {!tab.sendError && tab.responseTab === "cookies" && (
          <KeyValueReadout
            emptyLabel={t("api.response.noCookies")}
            items={responseCookies(tab.response)}
          />
        )}
        {!tab.sendError && tab.responseTab === "timing" && (
          <ResponseTiming response={tab.response} />
        )}
      </div>
    </section>
  );
}

function ResponseStatus({
  response,
  state,
}: {
  response: ApiResponse | null;
  state: ReturnType<typeof deriveTabResponseState>;
}) {
  const { t } = useI18n();

  if (!response) {
    const tone = state === "sending" ? "amber" : state === "idle" ? "neutral" : "red";
    return (
      <Badge className="h-[21px] gap-1.5 rounded-full px-2.5 text-[11px]" tone={tone}>
        {state === "sending" && <Loader2 className="animate-spin" size={11} />}
        {state === "idle" ? t("api.response.status.noResponse") : responseStateLabel(state, t)}
      </Badge>
    );
  }

  return (
    <Badge
      className="h-[21px] gap-1.5 rounded-full px-2.5 text-[11px]"
      tone={response.status < 400 ? "green" : "red"}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current" />
      {response.status} {response.statusText}
    </Badge>
  );
}

function ResponseBodyView({
  onOpenAuthSettings,
  response,
}: {
  onOpenAuthSettings: () => void;
  response: ApiResponse | null;
}) {
  const { t } = useI18n();
  const [mode, setMode] = useState<"pretty" | "raw">("pretty");

  if (!response) {
    return null;
  }

  if (response.status >= 400) {
    return (
      <ResponsePaneState
        description={t("api.response.rejectedDescription")}
        icon={<AlertCircle size={24} />}
        title={t("api.response.rejectedTitle")}
        tone="error"
      >
        {response.body.trim() && (
          <pre className="max-h-28 max-w-[min(520px,100%)] overflow-auto rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-3 py-2 text-left font-mono text-[12px] text-[var(--u-color-text-muted)]">
            {response.body}
          </pre>
        )}
        <Button onClick={onOpenAuthSettings} size="sm" type="button" variant="outline">
          {t("api.response.openAuthSettings")}
        </Button>
      </ResponsePaneState>
    );
  }

  if (!response.body.trim()) {
    return (
      <ResponsePaneState
        description={t("api.response.emptyBodyDescription")}
        icon={<CheckCircle2 size={24} />}
        title={t("api.response.emptyBodyTitle", {
          status: response.status,
        })}
        tone="ok"
      />
    );
  }

  const bodySize = new TextEncoder().encode(response.body).length;
  const isJson = looksLikeJson(response.body);
  const displayBody =
    mode === "pretty" && isJson ? formatResponseBody(response.body) : response.body;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex min-h-[32px] shrink-0 items-center justify-between border-b border-[var(--u-color-border)] px-3 py-1 text-[12px] text-[var(--u-color-text-muted)]">
        <span>
          {bodySize > 100_000
            ? t("api.response.longBody", { size: formatByteSize(bodySize) })
            : t("api.response.tabs.body")}
        </span>
        <div className="flex overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
          {(["pretty", "raw"] as const).map((item) => (
            <button
              aria-pressed={mode === item}
              className={cn(
                "h-6 px-2 text-[12px]",
                mode === item
                  ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
                  : "bg-[var(--u-color-bg)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
              )}
              key={item}
              onClick={() => setMode(item)}
              type="button"
            >
              {item === "pretty" ? t("api.response.pretty") : t("api.response.raw")}
            </button>
          ))}
        </div>
      </div>
      <div className="min-h-0 flex-1">
        <Editor
          defaultLanguage={isJson && mode === "pretty" ? "json" : "plaintext"}
          options={{
            fontSize: 12,
            minimap: { enabled: false },
            readOnly: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
          }}
          theme="vs-dark"
          value={displayBody}
        />
      </div>
    </div>
  );
}

function SendingState() {
  const { t } = useI18n();
  return (
    <div className="flex h-full flex-col gap-4 p-5">
      <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
        <Loader2 className="animate-spin text-[var(--u-color-primary)]" size={15} />
        {t("api.response.sendingDescription")}
      </div>
      <div className="flex max-w-xl animate-pulse flex-col gap-2">
        {[40, 86, 72, 80, 54, 68, 44].map((width) => (
          <div
            className="h-2.5 rounded-full bg-[var(--u-color-surface-muted)]"
            key={width}
            style={{ width: `${width}%` }}
          />
        ))}
      </div>
    </div>
  );
}

function ResponsePaneState({
  children,
  description,
  icon,
  title,
  tone,
}: {
  children?: ReactNode;
  description: ReactNode;
  icon: ReactNode;
  title: ReactNode;
  tone: "empty" | "error" | "ok";
}) {
  return (
    <div className="flex h-full min-h-0 flex-col items-center justify-center gap-3 p-8 text-center">
      <div
        className={cn(
          "grid h-12 w-12 place-items-center rounded-[14px]",
          tone === "empty" &&
            "bg-[var(--u-color-surface-muted)] text-[var(--u-color-text-soft)]",
          tone === "error" &&
            "bg-[var(--u-badge-danger-bg)] text-[var(--u-color-danger)]",
          tone === "ok" &&
            "bg-[var(--u-badge-success-bg)] text-[var(--u-color-success)]",
        )}
      >
        {icon}
      </div>
      <div className="space-y-1">
        <h3 className="m-0 text-[14px] font-semibold text-[var(--u-color-text)]">
          {title}
        </h3>
        <p className="m-0 max-w-[42ch] text-[12px] leading-5 text-[var(--u-color-text-muted)]">
          {description}
        </p>
      </div>
      {children}
    </div>
  );
}

function KeyValueReadout({
  emptyLabel,
  items,
}: {
  emptyLabel: string;
  items: KeyValue[];
}) {
  if (!items.length) {
    return <EmptyState className="m-3 h-[calc(100%-24px)]">{emptyLabel}</EmptyState>;
  }
  return (
    <div className="h-full overflow-auto p-2 text-[12px]">
      {items.map((item, index) => (
        <div
          className="grid min-h-[var(--u-size-table-row)] grid-cols-[minmax(120px,0.36fr)_minmax(0,1fr)] items-center gap-3 border-b border-[var(--u-color-border)] px-1"
          key={`${item.key}-${index}`}
        >
          <span className="truncate font-medium">{item.key}</span>
          <span className="truncate font-mono text-[var(--u-color-text-muted)]">
            {item.value}
          </span>
        </div>
      ))}
    </div>
  );
}

function ResponseTiming({ response }: { response: ApiResponse | null }) {
  const { t } = useI18n();

  if (!response) {
    return <EmptyState className="m-3">{t("api.response.noTiming")}</EmptyState>;
  }
  const bodyBytes = new TextEncoder().encode(response.body).length;
  return (
    <div className="grid gap-2 p-3 text-[12px] sm:grid-cols-2">
      <Metric label={t("api.response.metrics.total")} value={`${response.durationMs}ms`} />
      <Metric
        label={t("api.response.metrics.status")}
        value={`${response.status} ${response.statusText}`}
      />
      <Metric
        label={t("api.response.metrics.bodySize")}
        value={formatByteSize(bodyBytes)}
      />
      <Metric
        label={t("api.response.metrics.headers")}
        value={String(response.headers.length)}
      />
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 py-1">
      <div className="text-[10px] uppercase text-[var(--u-color-text-soft)]">{label}</div>
      <div className="truncate font-medium">{value}</div>
    </div>
  );
}

function responseCookies(response: ApiResponse | null) {
  return (
    response?.headers
      .filter((item) => item.key.toLowerCase() === "set-cookie")
      .flatMap((item) => parseSetCookieHeader(item.value)) ?? []
  );
}

function parseSetCookieHeader(value: string): KeyValue[] {
  const [pair] = value.split(";");
  const separator = pair.indexOf("=");
  if (separator < 0) {
    return [];
  }
  return [
    {
      enabled: true,
      key: pair.slice(0, separator).trim(),
      value: pair.slice(separator + 1).trim(),
    },
  ];
}

function responseStateLabel(
  state: ReturnType<typeof deriveTabResponseState>,
  t: (key: string) => string,
) {
  switch (state) {
    case "sending":
      return t("api.response.status.sending");
    case "network":
      return t("api.response.status.network");
    case "timeout":
      return t("api.response.status.timeout");
    case "failed":
      return t("api.response.status.failed");
    default:
      return state;
  }
}
