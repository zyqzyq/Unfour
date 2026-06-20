import { useState } from "react";
import { Folder, FolderOpen, FolderPlus, Plus, Search, Send } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  ContextMenuItem,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
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
  moveApiRequest,
  type ApiCollection,
  type ApiSavedRequest,
} from "@unfour/command-client";
import {
  collectTreeRequests,
  groupRequestsByCollection,
  parseKeyValues,
  type FolderNode,
} from "../request-utils";
import { methodBadgeLabel, methodToneClass } from "../model/request-tabs";
import type { ApiOpenIntent } from "../model/types";
import { useApiCollections } from "../hooks/useApiCollections";
import { ApiHistoryTree } from "./ApiHistoryTree";

type RequestMenuContext = {
  collections: ApiCollection[];
  duplicate: (requestId: string) => void;
  move: (request: ApiSavedRequest, collectionId: string | null) => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  remove: (requestId: string) => void;
  t: (key: string) => string;
};

type NameTarget =
  | { kind: "collection" }
  | { kind: "folder"; collectionId: string; parentPath: string };

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
  const [nameTarget, setNameTarget] = useState<NameTarget | null>(null);
  const [nameValue, setNameValue] = useState("");
  const [renameTarget, setRenameTarget] = useState<ApiCollection | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<ApiCollection | null>(null);
  const queryClient = useQueryClient();
  const { addFolderMut, collections, createMut, deleteMut, renameMut } =
    useApiCollections(workspaceId);
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
  const moveMutation = useMutation({
    mutationFn: ({
      collectionId,
      folderPath,
      requestId,
    }: {
      collectionId: string | null;
      folderPath: string | null;
      requestId: string;
    }) => moveApiRequest(workspaceId, requestId, collectionId, folderPath),
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

  const menuContext: RequestMenuContext = {
    collections,
    duplicate: duplicateMutation.mutate,
    move: (request, collectionId) =>
      moveMutation.mutate({
        collectionId,
        folderPath: request.folderPath,
        requestId: request.id,
      }),
    onOpenIntent,
    remove: deleteMutation.mutate,
    t,
  };

  const openFolderDialog = (collectionId: string, parentPath: string) => {
    setNameValue("");
    setNameTarget({ kind: "folder", collectionId, parentPath });
  };

  const addFolderAction = (collectionId: string, parentPath: string) => (
    <button
      aria-label={t("api.collection.addFolder")}
      className="grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] group-hover:opacity-100"
      onClick={(event) => {
        event.stopPropagation();
        openFolderDialog(collectionId, parentPath);
      }}
      title={t("api.collection.addFolder")}
      type="button"
    >
      <FolderPlus size={12} />
    </button>
  );

  function folderToTreeItem(
    node: FolderNode,
    collectionId: string | null,
  ): TreeViewItem {
    return {
      id: `folder:${collectionId ?? "unfiled"}:${node.path}`,
      icon: <Folder size={13} />,
      label: node.name,
      actions: collectionId ? addFolderAction(collectionId, node.path) : undefined,
      children: [
        ...node.folders.map((child) => folderToTreeItem(child, collectionId)),
        ...node.requests.map((request) => requestTreeItem(request, menuContext)),
      ],
    };
  }

  const collectionGroups = groupRequestsByCollection(
    savedRequests,
    collections,
    t("api.collection.unfiled"),
  );
  const collectionItems: TreeViewItem[] = collectionGroups.map((group) => ({
    id: `collection:${group.id ?? "unfiled"}`,
    icon: <FolderOpen size={13} />,
    label: group.name,
    meta: (
      <span className="text-[10px] tabular-nums text-[var(--u-color-text-soft)]">
        {collectTreeRequests(group.tree).length}
      </span>
    ),
    actions: group.collection
      ? addFolderAction(group.collection.id, "")
      : undefined,
    contextMenu: group.collection ? (
      <>
        <ContextMenuItem
          onSelect={() => {
            setRenameTarget(group.collection);
            setRenameValue(group.collection?.name ?? "");
          }}
        >
          {t("api.collection.rename")}
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() =>
            group.collection &&
            openFolderDialog(group.collection.id, "")
          }
        >
          {t("api.collection.addFolder")}
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() =>
            exportCollection(group.name, collectTreeRequests(group.tree))
          }
        >
          {t("api.collection.export")}
        </ContextMenuItem>
        <ContextMenuItem
          className="text-[var(--u-color-danger)]"
          onSelect={() => setDeleteTarget(group.collection)}
        >
          {t("api.collection.delete")}
        </ContextMenuItem>
      </>
    ) : undefined,
    children: [
      ...group.tree.folders.map((folder) => folderToTreeItem(folder, group.id)),
      ...group.tree.rootRequests.map((request) =>
        requestTreeItem(request, menuContext),
      ),
    ],
  }));

  // Re-key the tree on its expandable structure so newly created collections
  // and folders auto-expand (TreeView only reads defaultExpandedIds on mount).
  // Manual collapse of an unchanged structure is preserved (same key).
  const expandableIds = collectExpandableIds(collectionItems);

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
      <div className="flex shrink-0 items-center justify-between gap-2 px-2 pb-1">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("api.sidebar.collections")}
        </span>
        <button
          aria-label={t("api.collection.new")}
          className="grid h-6 w-6 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
          disabled={createMut.isPending}
          onClick={() => {
            setNameValue("");
            setNameTarget({ kind: "collection" });
          }}
          title={t("api.collection.new")}
          type="button"
        >
          <Plus size={14} />
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {collectionItems.length ? (
          <TreeView
            defaultExpandedIds={expandableIds}
            items={collectionItems}
            key={expandableIds.join("|")}
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
          <SidebarEmpty>{t("api.collection.none")}</SidebarEmpty>
        )}
      </div>
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

      <Dialog
        onOpenChange={(next) => !next && setNameTarget(null)}
        open={Boolean(nameTarget)}
      >
        <DialogContent
          title={
            nameTarget?.kind === "folder"
              ? t("api.collection.newFolder")
              : t("api.collection.newCollection")
          }
        >
          <DialogHeader>
            <DialogTitle>
              {nameTarget?.kind === "folder"
                ? t("api.collection.newFolder")
                : t("api.collection.newCollection")}
            </DialogTitle>
          </DialogHeader>
          <DialogBody>
            <Input
              autoFocus
              onChange={(event) => setNameValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  confirmName();
                }
              }}
              placeholder={t("api.collection.namePlaceholder")}
              value={nameValue}
            />
          </DialogBody>
          <DialogFooter>
            <Button onClick={() => setNameTarget(null)} type="button" variant="ghost">
              {t("api.save.cancel")}
            </Button>
            <Button
              disabled={
                !nameValue.trim() || createMut.isPending || addFolderMut.isPending
              }
              onClick={confirmName}
              type="button"
            >
              {t("api.save.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(next) => !next && setRenameTarget(null)}
        open={Boolean(renameTarget)}
      >
        <DialogContent title={t("api.collection.rename")}>
          <DialogHeader>
            <DialogTitle>{t("api.collection.rename")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <Input
              autoFocus
              onChange={(event) => setRenameValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  confirmRename();
                }
              }}
              value={renameValue}
            />
          </DialogBody>
          <DialogFooter>
            <Button onClick={() => setRenameTarget(null)} type="button" variant="ghost">
              {t("api.save.cancel")}
            </Button>
            <Button
              disabled={!renameValue.trim() || renameMut.isPending}
              onClick={confirmRename}
              type="button"
            >
              {t("api.save.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(next) => !next && setDeleteTarget(null)}
        open={Boolean(deleteTarget)}
      >
        <DialogContent title={t("api.collection.delete")}>
          <DialogHeader>
            <DialogTitle>{t("api.collection.delete")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <DialogDescription>
              {t("api.collection.deleteConfirm")}
            </DialogDescription>
          </DialogBody>
          <DialogFooter>
            <Button onClick={() => setDeleteTarget(null)} type="button" variant="ghost">
              {t("api.save.cancel")}
            </Button>
            <Button
              className="bg-[var(--u-color-danger)]"
              disabled={deleteMut.isPending}
              onClick={confirmDelete}
              type="button"
            >
              {t("api.collection.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );

  function confirmName() {
    const name = nameValue.trim();
    if (!nameTarget || !name) {
      return;
    }
    if (nameTarget.kind === "collection") {
      createMut.mutate(name, { onSuccess: () => setNameTarget(null) });
      return;
    }
    const folderPath = nameTarget.parentPath
      ? `${nameTarget.parentPath}/${name}`
      : name;
    addFolderMut.mutate(
      { collectionId: nameTarget.collectionId, folderPath },
      { onSuccess: () => setNameTarget(null) },
    );
  }

  function confirmRename() {
    const name = renameValue.trim();
    if (!renameTarget || !name) {
      return;
    }
    renameMut.mutate(
      { id: renameTarget.id, name },
      { onSuccess: () => setRenameTarget(null) },
    );
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    deleteMut.mutate(deleteTarget.id, { onSuccess: () => setDeleteTarget(null) });
  }
}

function collectExpandableIds(items: TreeViewItem[]): string[] {
  const ids: string[] = [];
  for (const item of items) {
    if (item.children?.length) {
      ids.push(item.id);
      ids.push(...collectExpandableIds(item.children));
    }
  }
  return ids;
}

function requestTreeItem(
  request: ApiSavedRequest,
  ctx: RequestMenuContext,
): TreeViewItem {
  const open = (action: "open" | "send" = "open") =>
    ctx.onOpenIntent({
      action,
      kind: "saved",
      nonce: Date.now(),
      requestId: request.id,
    });
  const moveTargets = ctx.collections.filter(
    (collection) => collection.id !== request.collectionId,
  );
  return {
    id: `request:${request.id}`,
    icon: <MethodMeta method={request.method} />,
    label: request.name,
    title: request.url,
    contextMenu: (
      <>
        <ContextMenuItem onSelect={() => open()}>Open</ContextMenuItem>
        <ContextMenuItem onSelect={() => open("send")}>Send</ContextMenuItem>
        <ContextMenuItem onSelect={() => ctx.duplicate(request.id)}>
          Duplicate
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() => void navigator.clipboard?.writeText(request.url)}
        >
          Copy URL
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => exportRequest(request)}>Export</ContextMenuItem>
        {(request.collectionId || moveTargets.length > 0) && (
          <ContextMenuItem disabled>{ctx.t("api.collection.moveTo")}</ContextMenuItem>
        )}
        {request.collectionId && (
          <ContextMenuItem onSelect={() => ctx.move(request, null)}>
            {ctx.t("api.collection.unfiled")}
          </ContextMenuItem>
        )}
        {moveTargets.map((collection) => (
          <ContextMenuItem
            key={collection.id}
            onSelect={() => ctx.move(request, collection.id)}
          >
            {collection.name}
          </ContextMenuItem>
        ))}
        <ContextMenuItem
          className="text-[var(--u-color-danger)]"
          onSelect={() => ctx.remove(request.id)}
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

function serializeRequest(request: ApiSavedRequest) {
  return {
    name: request.name,
    folderPath: request.folderPath,
    method: request.method,
    url: request.url,
    headers: parseKeyValues(request.headersJson),
    query: parseKeyValues(request.queryJson),
    body: request.body,
    bodyKind: request.bodyKind,
  };
}

function downloadJson(filename: string, value: unknown) {
  const href = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], {
      type: "application/json;charset=utf-8",
    }),
  );
  const link = document.createElement("a");
  link.href = href;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(href);
}

function exportRequest(request: ApiSavedRequest) {
  downloadJson(
    `${request.name.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}.json`,
    serializeRequest(request),
  );
}

function exportCollection(name: string, requests: ApiSavedRequest[]) {
  downloadJson(`${name.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}.json`, {
    name,
    savedRequests: requests.map(serializeRequest),
  });
}
