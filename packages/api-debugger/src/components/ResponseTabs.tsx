import type * as React from "react";
import Editor from "@monaco-editor/react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { Badge, Button, cn } from "@unfour/ui";
import type { ApiHistoryItem, ApiResponse, KeyValue } from "@unfour/command-client";
import { formatByteSize } from "../request-utils";
import {
  formatResponseBody,
  looksLikeJson,
} from "../model/api-request-state";
import type { ResponsePanelTab, ResponseTab } from "../model/types";
import { CompactTabs } from "./RequestParamsTabs";

const columnHelper = createColumnHelper<ApiHistoryItem>();

export function ResponseTabs({
  historyItems,
  loadingReplay,
  onReplay,
  onResponseTabChange,
  onResultTabChange,
  response,
  responseTab,
  resultTab,
  sending,
}: {
  historyItems: ApiHistoryItem[];
  loadingReplay: boolean;
  onReplay: (item: ApiHistoryItem) => void;
  onResponseTabChange: (tab: ResponseTab) => void;
  onResultTabChange: (tab: ResponsePanelTab) => void;
  response: ApiResponse | null;
  responseTab: ResponseTab;
  resultTab: ResponsePanelTab;
  sending: boolean;
}) {
  return (
    <section className="flex min-h-[260px] min-w-[320px] flex-[0.42] flex-col xl:min-h-0">
      <div className="flex h-[var(--u-size-section-toolbar)] shrink-0 items-center justify-between gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
        <div className="flex items-center gap-1">
          <SegmentButton
            active={resultTab === "response"}
            onClick={() => onResultTabChange("response")}
          >
            Response
          </SegmentButton>
          <SegmentButton
            active={resultTab === "history"}
            onClick={() => onResultTabChange("history")}
          >
            History
          </SegmentButton>
        </div>
        {resultTab === "response" ? (
          <ResponseStatus response={response} sending={sending} />
        ) : (
          <Badge tone="neutral">{historyItems.length} runs</Badge>
        )}
      </div>

      {resultTab === "response" ? (
        <>
          <CompactTabs
            active={responseTab}
            items={[
              { id: "body", label: "Body" },
              { id: "headers", label: "Headers", meta: response?.headers.length ?? 0 },
              { id: "cookies", label: "Cookies", meta: responseCookies(response).length },
              { id: "timing", label: "Timing" },
            ]}
            onChange={onResponseTabChange}
          />
          <div className="min-h-0 flex-1 overflow-hidden">
            {responseTab === "body" && <ResponseBodyView response={response} />}
            {responseTab === "headers" && (
              <KeyValueReadout emptyLabel="No response headers" items={response?.headers ?? []} />
            )}
            {responseTab === "cookies" && (
              <KeyValueReadout emptyLabel="No response cookies" items={responseCookies(response)} />
            )}
            {responseTab === "timing" && <ResponseTiming response={response} />}
          </div>
        </>
      ) : (
        <HistoryTable items={historyItems} loadingReplay={loadingReplay} onReplay={onReplay} />
      )}
    </section>
  );
}

function SegmentButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className={cn(
        "h-[26px] rounded-[var(--u-radius-sm)] px-2 text-[12px] font-medium transition-colors",
        active
          ? "bg-[var(--u-color-surface)] text-[var(--u-color-text)] ring-1 ring-inset ring-[var(--u-color-border)]"
          : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
      )}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}

function ResponseStatus({
  response,
  sending,
}: {
  response: ApiResponse | null;
  sending: boolean;
}) {
  if (sending) {
    return <Badge tone="amber">sending</Badge>;
  }
  if (!response) {
    return <Badge tone="neutral">no response</Badge>;
  }

  return (
    <div className="flex items-center gap-2">
      <Badge tone={response.status < 400 ? "green" : "red"}>{response.status}</Badge>
      <Badge tone="neutral">{response.durationMs}ms</Badge>
      <Badge tone="neutral">{formatByteSize(new TextEncoder().encode(response.body).length)}</Badge>
    </div>
  );
}

function ResponseBodyView({ response }: { response: ApiResponse | null }) {
  if (!response) {
    return <EmptyState className="h-full">Send a request to inspect the response</EmptyState>;
  }
  if (!response.body.trim()) {
    return <EmptyState className="h-full">Response body is empty</EmptyState>;
  }

  const language = looksLikeJson(response.body) ? "json" : "plaintext";
  const bodySize = new TextEncoder().encode(response.body).length;

  return (
    <div className="flex h-full min-h-0 flex-col">
      {bodySize > 100_000 && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-1 text-[12px] text-[var(--u-color-text-muted)]">
          Long response: {formatByteSize(bodySize)}
        </div>
      )}
      <div className="min-h-0 flex-1">
        <Editor
          defaultLanguage={language}
          options={{
            fontSize: 12,
            lineNumbersMinChars: 3,
            minimap: { enabled: false },
            readOnly: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
          }}
          theme="vs-light"
          value={formatResponseBody(response.body)}
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
  if (items.length === 0) {
    return <EmptyState className="h-full">{emptyLabel}</EmptyState>;
  }

  return (
    <div className="h-full min-h-0 overflow-auto p-2 text-[12px]">
      {items.map((item, index) => (
        <div
          className="grid min-h-[var(--u-size-table-row)] grid-cols-[minmax(120px,0.36fr)_minmax(0,1fr)] items-center gap-3 border-b border-[var(--u-color-border)] px-1"
          key={`${item.key}-${index}`}
        >
          <span className="min-w-0 truncate font-medium text-[var(--u-color-text)]">
            {item.key}
          </span>
          <span className="min-w-0 truncate font-mono text-[var(--u-color-text-muted)]">
            {item.value}
          </span>
        </div>
      ))}
    </div>
  );
}

function ResponseTiming({ response }: { response: ApiResponse | null }) {
  if (!response) {
    return <EmptyState className="h-full">No timing data</EmptyState>;
  }

  const bodyBytes = new TextEncoder().encode(response.body).length;
  const headerBytes = new TextEncoder().encode(
    response.headers.map((item) => `${item.key}: ${item.value}`).join("\r\n"),
  ).length;

  return (
    <div className="grid content-start gap-2 p-3 text-[12px] sm:grid-cols-2">
      <Metric label="Total" value={`${response.durationMs}ms`} />
      <Metric label="Status" value={`${response.status} ${response.statusText}`} />
      <Metric label="Body size" value={formatByteSize(bodyBytes)} />
      <Metric label="Header size" value={formatByteSize(headerBytes)} />
      <Metric label="Headers" value={String(response.headers.length)} />
      <Metric label="Cookies" value={String(responseCookies(response).length)} />
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 py-1">
      <div className="text-[10px] uppercase text-[var(--u-color-text-soft)]">{label}</div>
      <div className="truncate font-medium text-[var(--u-color-text)]">{value}</div>
    </div>
  );
}

function HistoryTable({
  items,
  loadingReplay,
  onReplay,
}: {
  items: ApiHistoryItem[];
  loadingReplay: boolean;
  onReplay: (item: ApiHistoryItem) => void;
}) {
  const columns = [
    columnHelper.display({
      cell: (info) => (
        <Button
          disabled={loadingReplay}
          onClick={() => onReplay(info.row.original)}
          size="sm"
          type="button"
          variant="ghost"
        >
          Load
        </Button>
      ),
      header: "",
      id: "replay",
    }),
    columnHelper.accessor("method", {
      cell: (info) => <Badge tone="teal">{info.getValue()}</Badge>,
      header: "Method",
    }),
    columnHelper.accessor("status", {
      cell: (info) => {
        const status = info.getValue();
        return status ? <Badge tone={status < 400 ? "green" : "red"}>{status}</Badge> : "-";
      },
      header: "Status",
    }),
    columnHelper.accessor("url", {
      cell: (info) => <span className="block max-w-[190px] truncate">{info.getValue()}</span>,
      header: "URL",
    }),
    columnHelper.accessor("durationMs", {
      cell: (info) => {
        const value = info.getValue();
        return value ? `${value}ms` : "-";
      },
      header: "Time",
    }),
  ];
  const table = useReactTable({
    columns,
    data: items,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <table className="data-table w-full text-left text-xs">
        <thead className="sticky top-0">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <th className="border-b border-slate-200 px-3 py-2 font-medium" key={header.id}>
                  {flexRender(header.column.columnDef.header, header.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr className="border-b" key={row.id}>
              {row.getVisibleCells().map((cell) => (
                <td className="px-3 py-2" key={cell.id}>
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {items.length === 0 && <EmptyState className="m-3 h-24">No requests yet</EmptyState>}
    </div>
  );
}

function responseCookies(response: ApiResponse | null) {
  return response?.headers.filter((item) => item.key.toLowerCase() === "set-cookie") ?? [];
}

function EmptyState({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "empty-state flex items-center justify-center rounded-md px-3 py-4 text-center text-sm",
        className,
      )}
    >
      {children}
    </div>
  );
}
