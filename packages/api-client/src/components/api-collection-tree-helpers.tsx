import type { ApiCollection, ApiSavedRequest } from "@unfour/command-client";
import { Folder, FolderOpen, FolderPlus } from "lucide-react";
import { ContextMenuItem, IconButton, type TreeViewItem } from "@unfour/ui";
import { collectTreeRequests, type ApiCollectionGroup, type FolderNode } from "../request-utils";
import { ApiCollectionMenu } from "./ApiCollectionMenu";
import { MethodMeta } from "./ApiTreeLabels";
import {
  RequestActionMenu,
  RequestContextMenu,
  type RequestTreeActionContext,
} from "./ApiRequestTreeActions";

type CollectionTreeActionContext = {
  addFolder: (collectionId: string, parentFolderId: string | null) => void;
  renameCollection: (collection: ApiCollection) => void;
  deleteCollection: (collection: ApiCollection) => void;
  exportCollection: (collection: ApiCollection) => void;
  renameFolder: (folder: FolderNode) => void;
  deleteFolder: (folder: FolderNode) => void;
  t: RequestTreeActionContext["t"];
};

export function collectionTreeItems(
  groups: ApiCollectionGroup[],
  requestContext: RequestTreeActionContext,
  context: CollectionTreeActionContext,
): TreeViewItem[] {
  return groups.map((group) => ({
    id: `collection:${group.id}`,
    icon: <FolderOpen size={13} />,
    label: group.name,
    meta: (
      <span className="text-[10px] tabular-nums text-[var(--u-color-text-soft)]">
        {collectTreeRequests(group.tree).length}
      </span>
    ),
    actions: (
      <div className="flex shrink-0 items-center gap-1">
        {addFolderAction(group.collection.id, null, context)}
        {collectionMenu(group.collection, context)}
      </div>
    ),
    contextMenu: collectionMenu(group.collection, context, true),
    children: [
      ...group.tree.folders.map((folder) => folderTreeItem(folder, group.id, requestContext, context)),
      ...group.tree.rootRequests.map((request) => requestTreeItem(request, requestContext)),
    ],
  }));
}

function folderTreeItem(
  node: FolderNode,
  collectionId: string,
  requestContext: RequestTreeActionContext,
  context: CollectionTreeActionContext,
): TreeViewItem {
  return {
    id: `folder:${node.id}`,
    icon: <Folder size={13} />,
    label: node.name,
    actions: addFolderAction(collectionId, node.id, context),
    contextMenu: (
      <>
        <ContextMenuItem onSelect={() => context.addFolder(collectionId, node.id)}>
          {context.t("api.collection.addFolder")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => context.renameFolder(node)}>
          {context.t("api.collection.renameFolder")}
        </ContextMenuItem>
        <ContextMenuItem className="text-[var(--u-color-danger)]" onSelect={() => context.deleteFolder(node)}>
          {context.t("api.collection.deleteFolder")}
        </ContextMenuItem>
      </>
    ),
    children: [
      ...node.folders.map((child) => folderTreeItem(child, collectionId, requestContext, context)),
      ...node.requests.map((request) => requestTreeItem(request, requestContext)),
    ],
  };
}

function addFolderAction(
  collectionId: string,
  parentFolderId: string | null,
  context: CollectionTreeActionContext,
) {
  return (
    <IconButton
      label={context.t("api.collection.addFolder")}
      size="compact"
      className="h-6 w-6"
      disableTooltip
      onClick={(event) => {
        event.stopPropagation();
        context.addFolder(collectionId, parentFolderId);
      }}
      title={context.t("api.collection.addFolder")}
      type="button"
    >
      <FolderPlus size={14} />
    </IconButton>
  );
}

function collectionMenu(collection: ApiCollection, context: CollectionTreeActionContext, isContextMenu = false) {
  return (
    <ApiCollectionMenu
      context={isContextMenu}
      name={collection.name}
      onRename={() => context.renameCollection(collection)}
      onAddFolder={() => context.addFolder(collection.id, null)}
      onExport={() => context.exportCollection(collection)}
      onDelete={() => context.deleteCollection(collection)}
    />
  );
}

export function collectExpandableIds(items: TreeViewItem[]): string[] {
  const ids: string[] = [];
  for (const item of items) {
    if (item.children?.length) {
      ids.push(item.id);
      ids.push(...collectExpandableIds(item.children));
    }
  }
  return ids;
}

export function requestTreeItem(
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
