import { useState } from "react";
import { Search, Send } from "lucide-react";
import { SidebarRow, TreeView, useI18n } from "@unfour/ui";
import { buildApiCollectionTree } from "../request-utils";
import type { ApiOpenIntent } from "../model/types";
import { useApiCollectionFolders } from "../hooks/useApiCollectionFolders";
import { useApiCollections } from "../hooks/useApiCollections";
import { useApiCollectionRequestActions } from "../hooks/useApiCollectionRequestActions";
import { useApiCollectionTreeData } from "../hooks/useApiCollectionTreeData";
import { useApiCollectionTreeDialogs } from "../hooks/useApiCollectionTreeDialogs";
import { useApiCollectionTreeDrop } from "../hooks/useApiCollectionTreeDrop";
import { ApiCollectionToolbarActions } from "./ApiCollectionToolbarActions";
import { ApiCollectionTreeDialogs } from "./ApiCollectionTreeDialogs";
import { ApiCollectionHistorySection } from "./ApiCollectionHistorySection";
import { SidebarEmpty } from "./ApiTreeLabels";
import { collectExpandableIds, collectionTreeItems } from "./api-collection-tree-helpers";

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
  const [importedId, setImportedId] = useState<string | null>(null);
  const collections = useApiCollections(workspaceId);
  const folderActions = useApiCollectionFolders(workspaceId);
  const requestActions = useApiCollectionRequestActions(workspaceId);
  const { requests, visibleRequests, historyItems } = useApiCollectionTreeData(
    workspaceId, search, folderActions.folders,
  );
  const dialogs = useApiCollectionTreeDialogs({
    collections, folders: folderActions, requestActions, requests,
  });
  const collectionGroups = buildApiCollectionTree(
    collections.collections, folderActions.folders, visibleRequests,
  );
  const drop = useApiCollectionTreeDrop({
    collectionGroups, folderActions, requests, visibleRequests, onError: dialogs.showError,
  });
  const items = collectionTreeItems(collectionGroups, {
    duplicate: requestActions.duplicateMutation.mutate,
    onOpenIntent,
    remove: requestActions.deleteMutation.mutate,
    rename: dialogs.renameRequestItem,
    t,
  }, {
    addFolder: dialogs.openFolder,
    renameCollection: dialogs.renameCollection,
    deleteCollection: dialogs.deleteCollectionItem,
    exportCollection: dialogs.exportCollectionItem,
    renameFolder: dialogs.renameFolderItem,
    deleteFolder: dialogs.deleteFolderItem,
    t,
  });
  // Re-key the tree on its expandable structure so newly created collections
  // and folders auto-expand (TreeView only reads defaultExpandedIds on mount).
  // Manual collapse of an unchanged structure is preserved (same key).
  const expandableIds = collectExpandableIds(items);

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
        <ApiCollectionToolbarActions
          key={workspaceId}
          onImported={(id) => { setSearch(""); setImportedId(`collection:${id}`); }}
          createPending={collections.createMut.isPending}
          onCreate={dialogs.openCollection}
          workspaceId={workspaceId}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {items.length ? (
          <TreeView
            canDrag={(item) => item.id.startsWith("request:") || item.id.startsWith("folder:")}
            canDrop={drop.canDrop}
            defaultExpandedIds={expandableIds}
            items={items}
            key={expandableIds.join("|")}
            onDrop={drop.onDrop}
            onSelect={(item) => {
              setImportedId(null);
              if (item.id.startsWith("request:")) {
                onOpenIntent({
                  kind: "saved",
                  nonce: Date.now(),
                  requestId: item.id.slice("request:".length),
                });
              }
            }}
            selectedId={importedId ?? (selectedId ? `request:${selectedId}` : null)}
          />
        ) : (
          <SidebarEmpty>{t("api.collection.none")}</SidebarEmpty>
        )}
      </div>
      <ApiCollectionHistorySection items={historyItems} onOpenIntent={onOpenIntent} />
      <ApiCollectionTreeDialogs dialogs={dialogs} workspaceId={workspaceId} />
    </div>
  );
}
