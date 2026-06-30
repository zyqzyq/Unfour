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
  type TreeViewDropPosition,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import {
  deleteApiRequest,
  duplicateApiRequest,
  listApiHistory,
  listSavedApiRequests,
  updateApiRequest,
  type ApiCollection,
  type ApiSavedRequest,
} from "@unfour/command-client";
import {
  buildApiCollectionTree,
  collectTreeRequests,
  parseKeyValues,
  savedRequestToInput,
  type FolderNode,
} from "../request-utils";
import { methodBadgeLabel, methodToneClass } from "../model/request-tabs";
import type { ApiOpenIntent } from "../model/types";
import { useApiCollectionFolders } from "../hooks/useApiCollectionFolders";
import { useApiCollections } from "../hooks/useApiCollections";
import { ApiHistoryTree } from "./ApiHistoryTree";
import {
  RequestActionMenu,
  RequestContextMenu,
  type RequestTreeActionContext,
} from "./ApiRequestTreeActions";
import { createApiCollectionDropController } from "./api-collection-dnd";

type NameTarget =
  | { kind: "collection" }
  | { kind: "folder"; collectionId: string; parentFolderId: string | null };

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
  const [renameFolderTarget, setRenameFolderTarget] = useState<FolderNode | null>(null);
  const [renameFolderValue, setRenameFolderValue] = useState("");
  const [renameRequestTarget, setRenameRequestTarget] = useState<ApiSavedRequest | null>(null);
  const [renameRequestValue, setRenameRequestValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<ApiCollection | null>(null);
  const [deleteFolderTarget, setDeleteFolderTarget] = useState<FolderNode | null>(null);
  const queryClient = useQueryClient();
  const { collections, createMut, deleteMut, renameMut } = useApiCollections(workspaceId);
  const {
    createFolderMut,
    deleteFolderMut,
    folders,
    moveRequestMut,
    renameFolderMut,
    reorderRequestsMut,
  } = useApiCollectionFolders(workspaceId);
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
  const updateRequestMutation = useMutation({
    mutationFn: ({
      name,
      request,
    }: {
      name: string;
      request: ApiSavedRequest;
    }) =>
      updateApiRequest(workspaceId, request.id, {
        ...savedRequestToInput(request, workspaceId),
        name,
      }),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const folderNameById = new Map(folders.map((folder) => [folder.id, folder.name]));
  const searchText = search.trim().toLowerCase();
  const savedRequests = (savedQuery.data ?? []).filter((request) =>
    searchText
      ? [
          request.name,
          request.url,
          request.method,
          request.parentFolderId
            ? (folderNameById.get(request.parentFolderId) ?? "")
            : "",
        ].some((value) => value.toLowerCase().includes(searchText))
      : true,
  );
  const historyItems = (historyQuery.data ?? []).filter((item) =>
    searchText
      ? [item.name ?? "", item.url, item.method, String(item.status ?? "")]
          .some((value) => value.toLowerCase().includes(searchText))
      : true,
  );

  const menuContext: RequestTreeActionContext = {
    duplicate: duplicateMutation.mutate,
    exportRequest,
    onOpenIntent,
    remove: deleteMutation.mutate,
    rename: (request) => {
      setRenameRequestTarget(request);
      setRenameRequestValue(request.name);
    },
    t,
  };

  const openFolderDialog = (
    collectionId: string,
    parentFolderId: string | null,
  ) => {
    setNameValue("");
    setNameTarget({ kind: "folder", collectionId, parentFolderId });
  };

  const addFolderAction = (
    collectionId: string,
    parentFolderId: string | null,
  ) => (
    <button
      aria-label={t("api.collection.addFolder")}
      className="grid h-4 w-4 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] group-hover:opacity-100"
      onClick={(event) => {
        event.stopPropagation();
        openFolderDialog(collectionId, parentFolderId);
      }}
      title={t("api.collection.addFolder")}
      type="button"
    >
      <FolderPlus size={11} />
    </button>
  );

  function folderToTreeItem(
    node: FolderNode,
    collectionId: string,
  ): TreeViewItem {
    return {
      id: `folder:${node.id}`,
      icon: <Folder size={13} />,
      label: node.name,
      actions: addFolderAction(collectionId, node.id),
      contextMenu: (
        <>
          <ContextMenuItem onSelect={() => openFolderDialog(collectionId, node.id)}>
            {t("api.collection.addFolder")}
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => {
              setRenameFolderTarget(node);
              setRenameFolderValue(node.name);
            }}
          >
            {t("api.collection.renameFolder")}
          </ContextMenuItem>
          <ContextMenuItem
            className="text-[var(--u-color-danger)]"
            onSelect={() => setDeleteFolderTarget(node)}
          >
            {t("api.collection.deleteFolder")}
          </ContextMenuItem>
        </>
      ),
      children: [
        ...node.folders.map((child) => folderToTreeItem(child, collectionId)),
        ...node.requests.map((request) => requestTreeItem(request, menuContext)),
      ],
    };
  }

  const collectionGroups = buildApiCollectionTree(
    collections,
    folders,
    savedRequests,
  );
  const dropController = createApiCollectionDropController(
    collectionGroups,
    folders,
    savedRequests,
  );
  const collectionItems: TreeViewItem[] = collectionGroups.map((group) => ({
    id: `collection:${group.id}`,
    icon: <FolderOpen size={13} />,
    label: group.name,
    meta: (
      <span className="text-[10px] tabular-nums text-[var(--u-color-text-soft)]">
        {collectTreeRequests(group.tree).length}
      </span>
    ),
    actions: addFolderAction(group.collection.id, null),
    contextMenu: (
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
            group.collection && openFolderDialog(group.collection.id, null)
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
    ),
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
            canDrag={(item) => item.id.startsWith("request:")}
            canDrop={dropController.canDrop}
            defaultExpandedIds={expandableIds}
            items={collectionItems}
            key={expandableIds.join("|")}
            onDrop={({ position, source, target }) =>
              moveDroppedTreeItem(source, target, position)
            }
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
                !nameValue.trim() || createMut.isPending || createFolderMut.isPending
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
        onOpenChange={(next) => !next && setRenameFolderTarget(null)}
        open={Boolean(renameFolderTarget)}
      >
        <DialogContent title={t("api.collection.renameFolder")}>
          <DialogHeader>
            <DialogTitle>{t("api.collection.renameFolder")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <Input
              autoFocus
              onChange={(event) => setRenameFolderValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  confirmFolderRename();
                }
              }}
              value={renameFolderValue}
            />
          </DialogBody>
          <DialogFooter>
            <Button
              onClick={() => setRenameFolderTarget(null)}
              type="button"
              variant="ghost"
            >
              {t("api.save.cancel")}
            </Button>
            <Button
              disabled={!renameFolderValue.trim() || renameFolderMut.isPending}
              onClick={confirmFolderRename}
              type="button"
            >
              {t("api.save.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(next) => !next && setRenameRequestTarget(null)}
        open={Boolean(renameRequestTarget)}
      >
        <DialogContent title={t("api.request.renameTitle")}>
          <DialogHeader>
            <DialogTitle>{t("api.request.renameTitle")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <Input
              autoFocus
              onChange={(event) => setRenameRequestValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  confirmRequestRename();
                }
              }}
              value={renameRequestValue}
            />
          </DialogBody>
          <DialogFooter>
            <Button onClick={() => setRenameRequestTarget(null)} type="button" variant="ghost">
              {t("api.save.cancel")}
            </Button>
            <Button
              disabled={!renameRequestValue.trim() || updateRequestMutation.isPending}
              onClick={confirmRequestRename}
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

      <Dialog
        onOpenChange={(next) => !next && setDeleteFolderTarget(null)}
        open={Boolean(deleteFolderTarget)}
      >
        <DialogContent title={t("api.collection.deleteFolder")}>
          <DialogHeader>
            <DialogTitle>{t("api.collection.deleteFolder")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <DialogDescription>
              {t("api.collection.deleteFolderConfirm")}
            </DialogDescription>
          </DialogBody>
          <DialogFooter>
            <Button
              onClick={() => setDeleteFolderTarget(null)}
              type="button"
              variant="ghost"
            >
              {t("api.save.cancel")}
            </Button>
            <Button
              className="bg-[var(--u-color-danger)]"
              disabled={deleteFolderMut.isPending}
              onClick={confirmFolderDelete}
              type="button"
            >
              {t("api.collection.deleteFolder")}
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
    createFolderMut.mutate(
      {
        collectionId: nameTarget.collectionId,
        name,
        parentFolderId: nameTarget.parentFolderId,
      },
      { onSuccess: () => setNameTarget(null) },
    );
  }

  function moveDroppedTreeItem(
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) {
    const action = dropController.dropAction(source, target, position);
    if (!action) {
      return;
    }
    switch (action.kind) {
      case "move-request":
        moveRequestMut.mutate({
          collectionId: action.collectionId,
          parentFolderId: action.parentFolderId,
          requestId: action.requestId,
        });
        break;
      case "reorder-requests":
        reorderRequestsMut.mutate({
          collectionId: action.collectionId,
          parentFolderId: action.parentFolderId,
          requestIds: action.requestIds,
        });
        break;
    }
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

  function confirmFolderRename() {
    const name = renameFolderValue.trim();
    if (!renameFolderTarget || !name) {
      return;
    }
    renameFolderMut.mutate(
      { folderId: renameFolderTarget.id, name },
      { onSuccess: () => setRenameFolderTarget(null) },
    );
  }

  function confirmRequestRename() {
    const name = renameRequestValue.trim();
    if (!renameRequestTarget || !name) {
      return;
    }
    updateRequestMutation.mutate(
      { name, request: renameRequestTarget },
      { onSuccess: () => setRenameRequestTarget(null) },
    );
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    deleteMut.mutate(deleteTarget.id, { onSuccess: () => setDeleteTarget(null) });
  }

  function confirmFolderDelete() {
    if (!deleteFolderTarget) {
      return;
    }
    deleteFolderMut.mutate(deleteFolderTarget.id, {
      onSuccess: () => setDeleteFolderTarget(null),
    });
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
  ctx: RequestTreeActionContext,
): TreeViewItem {
  return {
    id: `request:${request.id}`,
    label: (
      <span className="flex min-w-0 items-center gap-1.5">
        <MethodMeta method={request.method} />
        <span className="min-w-0 truncate">{request.name}</span>
      </span>
    ),
    title: request.url,
    actions: <RequestActionMenu ctx={ctx} request={request} />,
    contextMenu: <RequestContextMenu ctx={ctx} request={request} />,
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
    collectionId: request.collectionId,
    parentFolderId: request.parentFolderId,
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
