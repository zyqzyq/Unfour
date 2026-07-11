import { useState, type ReactNode } from "react";
import Editor from "@monaco-editor/react";
import { AlertCircle, CheckCircle2, Loader2 } from "lucide-react";
import { Badge, Button, EmptyState, cn, useI18n, useTheme } from "@unfour/ui";
import type { ApiRequestInput, ApiResponse, KeyValue } from "@unfour/command-client";
import { formatByteSize, isSensitiveKey } from "../request-utils";
import { formatResponseBody, looksLikeJson } from "../model/api-request-state";
import { deriveTabResponseState } from "../model/request-tabs";

export function ResponseStatus({
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

export function ResponseBodyView({
  onOpenAuthSettings,
  response,
}: {
  onOpenAuthSettings: () => void;
  response: ApiResponse | null;
}) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const [mode, setMode] = useState<"pretty" | "raw">("pretty");

  if (!response) {
    return null;
  }

  if (response.status >= 400) {
    const body = response.body.trim();
    const displayBody = looksLikeJson(body) ? formatResponseBody(body) : body;

    return (
      <ResponsePaneState
        description={t("api.response.rejectedDescription")}
        icon={<AlertCircle size={24} />}
        title={t("api.response.rejectedTitle")}
        tone="error"
      >
        {body && (
          <pre className="max-h-60 w-full max-w-3xl overflow-auto whitespace-pre-wrap break-words rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-3 py-2 text-left font-mono text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {displayBody}
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
          theme={theme === "dark" ? "unfour-dark" : "unfour-light"}
          value={displayBody}
        />
      </div>
    </div>
  );
}

export function RequestSnapshot({ request }: { request: ApiRequestInput | null }) {
  const { t } = useI18n();

  if (!request) {
    return <EmptyState className="m-3 h-[calc(100%-24px)]">{t("api.response.noRequest")}</EmptyState>;
  }

  const headers = redactKeyValues(request.headers);
  const query = redactKeyValues(request.query);

  return (
    <div className="h-full overflow-auto p-3 text-[12px] text-[var(--u-color-text)]">
      <div className="mb-3 rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2">
        <div className="mb-1 text-[10px] font-semibold uppercase tracking-[0.06em] text-[var(--u-color-text-soft)]">
          {t("api.response.request.summary")}
        </div>
        <div className="flex min-w-0 items-center gap-2">
          <span className="rounded-[var(--u-radius-sm)] bg-[var(--u-color-primary-soft)] px-1.5 py-0.5 font-mono text-[11px] font-semibold text-[var(--u-color-primary)]">
            {request.method}
          </span>
          <span className="min-w-0 break-all font-mono text-[var(--u-color-text-muted)]">
            {request.url}
          </span>
        </div>
      </div>
      <RequestKeyValueReadout
        emptyLabel={t("api.keyValue.empty")}
        items={query}
        title={t("api.response.request.query")}
      />
      <RequestKeyValueReadout
        emptyLabel={t("api.response.noHeaders")}
        items={headers}
        title={t("api.response.request.headers")}
      />
      <section className="mt-3">
        <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-[0.06em] text-[var(--u-color-text-soft)]">
          {t("api.response.request.body")}
        </h4>
        {request.body?.trim() ? (
          <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-3 py-2 font-mono text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {request.body}
          </pre>
        ) : (
          <div className="rounded-[var(--u-radius-sm)] border border-dashed border-[var(--u-color-border)] px-3 py-2 text-[var(--u-color-text-soft)]">
            {t("api.keyValue.empty")}
          </div>
        )}
      </section>
    </div>
  );
}

export function RequestKeyValueReadout({
  emptyLabel,
  items,
  title,
}: {
  emptyLabel: string;
  items: KeyValue[];
  title: string;
}) {
  return (
    <section className="mt-3">
      <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-[0.06em] text-[var(--u-color-text-soft)]">
        {title}
      </h4>
      {items.length ? (
        <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
          {items.map((item, index) => (
            <div
              className="grid min-h-[var(--u-size-table-row)] grid-cols-[minmax(120px,0.32fr)_minmax(0,1fr)] items-start gap-3 border-b border-[var(--u-color-border)] px-2 py-1.5 last:border-b-0"
              key={`${item.key}-${index}`}
            >
              <span className="break-all font-medium">{item.key}</span>
              <span className="break-all font-mono text-[var(--u-color-text-muted)]">
                {item.value}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className="rounded-[var(--u-radius-sm)] border border-dashed border-[var(--u-color-border)] px-3 py-2 text-[var(--u-color-text-soft)]">
          {emptyLabel}
        </div>
      )}
    </section>
  );
}

export function redactKeyValues(items: KeyValue[]): KeyValue[] {
  return items.map((item) =>
    isSensitiveKey(item.key) ? { ...item, value: "<redacted>" } : item,
  );
}

export function SendingState() {
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

export function ResponsePaneState({
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

export function KeyValueReadout({
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

export function ResponseTiming({ response }: { response: ApiResponse | null }) {
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

export function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 py-1">
      <div className="text-[10px] uppercase text-[var(--u-color-text-soft)]">{label}</div>
      <div className="truncate font-medium">{value}</div>
    </div>
  );
}

export function responseCookies(response: ApiResponse | null) {
  return (
    response?.headers
      .filter((item) => item.key.toLowerCase() === "set-cookie")
      .flatMap((item) => parseSetCookieHeader(item.value)) ?? []
  );
}

export function parseSetCookieHeader(value: string): KeyValue[] {
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

export function responseStateLabel(
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
