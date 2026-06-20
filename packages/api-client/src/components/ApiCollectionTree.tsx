import { useState } from "react";
import { Folder, Search, Send } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ContextMenuItem,
  SidebarRow,
  SidebarSection,
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import {
  deleteApiRequest,
  duplicateApiRequest,
  listApiHistory,
  listSavedApiRequests,
  type ApiSavedRequest,
} from "@unfour/command-client";
import { groupSavedRequests, parseKeyValues } from "../request-utils";
import { methodBadgeLabel, methodToneClass } from "../model/request-tabs";
import type { ApiOpenIntent } from "../model/types";
import { ApiHistoryTree } from "./ApiHistoryTree";

export function ApiCollectionTree({
  active,
  collapsed,
  onOpenClient,
  onOpenIntent,
  selectedId,
  workspaceId,
}: {
  active: boolean;
  collapsed: boolean;
  onOpenClient: () => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  selectedId: string | null;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const [search, setSearch] = useState("");
  const queryClient = useQueryClient();
  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });
  const duplicateMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });
  const deleteMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });
  const searchText = search.trim().toLowerCase();
  const savedRequests = (savedQuery.data ?? []).filter((request) =>
    searchText
      ? [
          request.name,
          request.url,
          request.method,
          request.folderPath ?? "",
        ].some((value) => value.toLowerCase().includes(searchText))
      : true,
  );
  const historyItems = (historyQuery.data ?? []).filter((item) =>
    searchText
      ? [item.name ?? "", item.url, item.method, String(item.status ?? "")]
          .some((value) => value.toLowerCase().includes(searchText))
      : true,
  );
  const collectionItems: TreeViewItem[] = groupSavedRequests(savedRequests).map((group) => ({
    id: `folder:${group.folder}`,
    icon: <Folder size={13} />,
    label: group.folder,
    children: group.items.map((request) =>
      requestTreeItem(
        request,
        onOpenIntent,
        duplicateMutation.mutate,
        deleteMutation.mutate,
      ),
    ),
  }));

  if (collapsed) {
    return (
      <SidebarRow active={active} onClick={onOpenClient} title={t("api.sidebar.restClient")}>
        <Send size={14} />
      </SidebarRow>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="px-2 py-2">
        <label className="flex h-7 items-center gap-2 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[12px] text-[var(--u-color-text-muted)]">
          <Search size={14} />
          <input
            aria-label={t("api.sidebar.searchAria")}
            className="min-w-0 flex-1 bg-transparent text-[12px] text-[var(--u-color-text)] outline-none placeholder:text-[var(--u-color-text-soft)]"
            onChange={(event) => setSearch(event.target.value)}
            placeholder={t("api.sidebar.searchPlaceholder")}
            value={search}
          />
        </label>
      </div>
      <SidebarSection className="min-h-0 flex-1 overflow-y-auto px-2 pb-2" title={t("api.sidebar.collections")}>
        {collectionItems.length ? (
          <TreeView
            defaultExpandedIds={collectionItems.map((item) => item.id)}
            items={collectionItems}
            onSelect={(item) => {
              if (item.id.startsWith("request:")) {
                onOpenIntent({
                  kind: "saved",
                  nonce: Date.now(),
                  requestId: item.id.slice("request:".length),
                });
              }
            }}
            selectedId={selectedId ? `request:${selectedId}` : null}
          />
        ) : (
          <SidebarEmpty>{t("api.sidebar.noSavedRequests")}</SidebarEmpty>
        )}
      </SidebarSection>
      <SidebarSection
        className="max-h-[220px] shrink-0 overflow-y-auto border-t border-[var(--u-color-border)] px-2 pb-2 pt-2"
        title={t("api.sidebar.history")}
      >
        {historyItems.length > 0 ? (
          <ApiHistoryTree
            items={historyItems}
            onOpenIntent={onOpenIntent}
          />
        ) : (
          <SidebarEmpty>{t("api.sidebar.historyEmptyCompact")}</SidebarEmpty>
        )}
      </SidebarSection>
    </div>
  );
}

function requestTreeItem(
  request: ApiSavedRequest,
  onOpenIntent: (intent: ApiOpenIntent) => void,
  duplicate: (requestId: string) => void,
  remove: (requestId: string) => void,
): TreeViewItem {
  const open = (action: "open" | "send" = "open") =>
    onOpenIntent({
      action,
      kind: "saved",
      nonce: Date.now(),
      requestId: request.id,
    });
  return {
    id: `request:${request.id}`,
    icon: <MethodMeta method={request.method} />,
    label: request.name,
    title: request.url,
    contextMenu: (
      <>
        <ContextMenuItem onSelect={() => open()}>Open</ContextMenuItem>
        <ContextMenuItem disabled>Open in New Tab (unique tab)</ContextMenuItem>
        <ContextMenuItem onSelect={() => open("send")}>Send</ContextMenuItem>
        <ContextMenuItem disabled>Rename (not available in this phase)</ContextMenuItem>
        <ContextMenuItem onSelect={() => duplicate(request.id)}>
          Duplicate
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() => void navigator.clipboard?.writeText(request.url)}
        >
          Copy URL
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => exportRequest(request)}>Export</ContextMenuItem>
        <ContextMenuItem
          className="text-[var(--u-color-danger)]"
          onSelect={() => remove(request.id)}
        >
          Delete
        </ContextMenuItem>
      </>
    ),
  };
}

function MethodMeta({ method }: { method: string }) {
  return (
    <span
      className={`w-9 shrink-0 text-left text-[10px] font-bold uppercase tabular-nums ${methodToneClass(method)}`}
    >
      {methodBadgeLabel(method)}
    </span>
  );
}

function SidebarEmpty({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
      {children}
    </div>
  );
}

function exportRequest(request: ApiSavedRequest) {
  const value = {
    name: request.name,
    folderPath: request.folderPath,
    method: request.method,
    url: request.url,
    headers: parseKeyValues(request.headersJson),
    query: parseKeyValues(request.queryJson),
    body: request.body,
    bodyKind: request.bodyKind,
  };
  const href = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], {
      type: "application/json;charset=utf-8",
    }),
  );
  const link = document.createElement("a");
  link.href = href;
  link.download = `${request.name.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}.json`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(href);
}
