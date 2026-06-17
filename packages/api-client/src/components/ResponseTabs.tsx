import { useState } from "react";
import Editor from "@monaco-editor/react";
import { Loader2 } from "lucide-react";
import { Badge, EmptyState, ErrorState } from "@unfour/ui";
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
  onResponseTabChange,
  tab,
}: {
  onResponseTabChange: (tab: ResponseTab) => void;
  tab: ApiRequestTab;
}) {
  const responseState = deriveTabResponseState(tab);
  return (
    <section className="relative flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-end justify-between border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
        <CompactTabs
          active={tab.responseTab}
          className="border-b-0 bg-transparent"
          items={[
            { id: "body", label: "Body" },
            { id: "headers", label: "Headers", meta: tab.response?.headers.length ?? 0 },
            {
              id: "cookies",
              label: "Cookies",
              meta: responseCookies(tab.response).length,
            },
            { id: "timing", label: "Timing" },
          ]}
          onChange={onResponseTabChange}
        />
        <div className="flex h-full shrink-0 items-center px-2">
          <ResponseStatus response={tab.response} state={responseState} />
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {responseState === "sending" && (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-[var(--u-color-text-muted)]">
            <Loader2 className="animate-spin text-[var(--u-color-primary)]" size={24} />
            <span className="text-[13px]">Sending request...</span>
          </div>
        )}
        {(responseState === "network" ||
          responseState === "timeout" ||
          responseState === "failed") && (
          <ErrorState className="m-3 h-[calc(100%-24px)]">
            {tab.sendError}
          </ErrorState>
        )}
        {responseState !== "sending" &&
          !tab.sendError &&
          tab.responseTab === "body" && <ResponseBodyView response={tab.response} />}
        {!tab.sendError && tab.responseTab === "headers" && (
          <KeyValueReadout
            emptyLabel="No response headers"
            items={tab.response?.headers ?? []}
          />
        )}
        {!tab.sendError && tab.responseTab === "cookies" && (
          <KeyValueReadout
            emptyLabel="No response cookies"
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
  if (!response) {
    return <Badge tone={state === "sending" ? "amber" : state === "idle" ? "neutral" : "red"}>{state === "idle" ? "no response" : state}</Badge>;
  }
  return (
    <div className="flex items-center gap-1.5">
      <Badge tone={response.status < 400 ? "green" : "red"}>
        {response.status} {response.statusText}
      </Badge>
      <Badge tone="neutral">{response.durationMs}ms</Badge>
      <Badge tone="neutral">
        {formatByteSize(new TextEncoder().encode(response.body).length)}
      </Badge>
    </div>
  );
}

function ResponseBodyView({ response }: { response: ApiResponse | null }) {
  const [mode, setMode] = useState<"pretty" | "raw">("pretty");
  if (!response) {
    return (
      <div className="flex h-full items-center justify-center text-[13px] text-[var(--u-color-text-muted)]">
        Send a request to inspect the response
      </div>
    );
  }
  if (!response.body.trim()) {
    return (
      <EmptyState className="m-3 h-[calc(100%-24px)]">
        Empty response
      </EmptyState>
    );
  }
  const bodySize = new TextEncoder().encode(response.body).length;
  const isJson = looksLikeJson(response.body);
  const displayBody = mode === "pretty" && isJson
    ? formatResponseBody(response.body)
    : response.body;
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex min-h-[32px] shrink-0 items-center justify-between border-b border-[var(--u-color-border)] px-3 py-1 text-[12px] text-[var(--u-color-text-muted)]">
        <span>{bodySize > 100_000 ? `Long response: ${formatByteSize(bodySize)}` : "Body"}</span>
        <div className="flex overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
          {(["pretty", "raw"] as const).map((item) => (
            <button
              aria-pressed={mode === item}
              className={
                mode === item
                  ? "h-6 bg-[var(--u-color-surface-active)] px-2 text-[var(--u-color-text)]"
                  : "h-6 bg-[var(--u-color-bg)] px-2 text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
              }
              key={item}
              onClick={() => setMode(item)}
              type="button"
            >
              {item === "pretty" ? "Pretty" : "Raw"}
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

function KeyValueReadout({
  emptyLabel,
  items,
}: {
  emptyLabel: string;
  items: KeyValue[];
}) {
  if (!items.length) {
    return <EmptyState className="m-3">{emptyLabel}</EmptyState>;
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
  if (!response) {
    return <EmptyState className="m-3">No timing data</EmptyState>;
  }
  const bodyBytes = new TextEncoder().encode(response.body).length;
  return (
    <div className="grid gap-2 p-3 text-[12px] sm:grid-cols-2">
      <Metric label="Total" value={`${response.durationMs}ms`} />
      <Metric label="Status" value={`${response.status} ${response.statusText}`} />
      <Metric label="Body size" value={formatByteSize(bodyBytes)} />
      <Metric label="Headers" value={String(response.headers.length)} />
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
