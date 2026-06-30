import { useState } from "react";
import { Folder, FolderOpen } from "lucide-react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiSavedRequest,
} from "@unfour/command-client";
import { buildApiCollectionTree, type FolderNode } from "../request-utils";

export type SaveIdentity = {
  collectionId: string | null;
  createCollectionName?: string;
  name: string;
  newFolderName?: string;
  parentFolderId: string | null;
};

type Target = { collectionId: string; parentFolderId: string | null };

function targetId(collectionId: string, parentFolderId: string | null) {
  return `pick:${collectionId}:${parentFolderId ?? "root"}`;
}

export function ApiSaveDialog({
  collections,
  defaultCollectionId,
  defaultParentFolderId,
  defaultName,
  folders,
  onCancel,
  onSave,
  open,
  savedRequests,
  saving,
}: {
  collections: ApiCollection[];
  defaultCollectionId: string | null;
  defaultParentFolderId: string | null;
  defaultName: string;
  folders: ApiCollectionFolder[];
  onCancel: () => void;
  onSave: (identity: SaveIdentity) => void;
  open: boolean;
  savedRequests: ApiSavedRequest[];
  saving: boolean;
}) {
  const { t } = useI18n();

  const targets = new Map<string, Target>();
  const registerTarget = (collectionId: string, parentFolderId: string | null) => {
    const id = targetId(collectionId, parentFolderId);
    targets.set(id, { collectionId, parentFolderId });
    return id;
  };

  function folderItem(node: FolderNode, collectionId: string): TreeViewItem {
    return {
      id: registerTarget(collectionId, node.id),
      icon: <Folder size={13} />,
      label: node.name,
      children: node.folders.map((child) => folderItem(child, collectionId)),
    };
  }

  const groups = buildApiCollectionTree(collections, folders, savedRequests);
  const items: TreeViewItem[] = groups.length
    ? groups.map<TreeViewItem>((group) => ({
        id: registerTarget(group.id, null),
        icon: <FolderOpen size={13} />,
        label: group.name,
        children: group.tree.folders.map((folder) =>
          folderItem(folder, group.id),
        ),
      }))
    : [
        {
          id: registerTarget("__default__", null),
          icon: <FolderOpen size={13} />,
          label: t("api.collection.defaultCollection"),
          children: [],
        },
      ];

  const defaultId = defaultCollectionId
    ? targetId(defaultCollectionId, defaultParentFolderId)
    : "";
  const autoSelectedId =
    targets.has(defaultId) ? defaultId : items[0]?.id ?? "";

  const [name, setName] = useState(defaultName);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [subfolder, setSubfolder] = useState("");
  const [creatingNew, setCreatingNew] = useState(false);
  const [newCollectionName, setNewCollectionName] = useState("");
  const effectiveSelectedId =
    selectedId && targets.has(selectedId) ? selectedId : autoSelectedId;

  const canSave =
    Boolean(name.trim()) &&
    !saving &&
    (!creatingNew || Boolean(newCollectionName.trim()));

  function handleSave() {
    const sub = subfolder.trim();
    if (creatingNew) {
      onSave({
        collectionId: null,
        createCollectionName: newCollectionName.trim(),
        name: name.trim(),
        newFolderName: sub || undefined,
        parentFolderId: null,
      });
      return;
    }
    const target = targets.get(effectiveSelectedId);
    // __default__ means let backend create default collection
    const isDefaultCollection = target?.collectionId === "__default__";
    const collectionId = isDefaultCollection
      ? null
      : (target?.collectionId ?? collections[0]?.id ?? null);
    onSave({
      collectionId,
      name: name.trim(),
      newFolderName: sub || undefined,
      parentFolderId: target?.parentFolderId ?? null,
    });
  }

  return (
    <Dialog onOpenChange={(next) => !next && onCancel()} open={open}>
      <DialogContent title={t("api.save.title")}>
        <DialogHeader>
          <DialogTitle>{t("api.save.title")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="space-y-3">
          <label className="block space-y-1">
            <span className="text-[12px] font-medium">{t("api.save.name")}</span>
            <Input
              autoFocus
              onChange={(event) => setName(event.target.value)}
              value={name}
            />
          </label>

          <div className="space-y-1">
            <span className="text-[12px] font-medium">{t("api.save.location")}</span>
            <div className="max-h-[200px] overflow-y-auto rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-1">
              <TreeView
                defaultExpandedIds={collectExpandableIds(items)}
                items={items}
                onSelect={(item) => {
                  if (targets.has(item.id)) {
                    setCreatingNew(false);
                    setSelectedId(item.id);
                  }
                }}
                selectedId={creatingNew ? null : effectiveSelectedId}
              />
            </div>
            <button
              className={`text-[12px] ${
                creatingNew
                  ? "font-semibold text-[var(--u-color-primary)]"
                  : "text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
              }`}
              onClick={() => setCreatingNew((value) => !value)}
              type="button"
            >
              {t("api.save.newCollection")}
            </button>
          </div>

          {creatingNew ? (
            <label className="block space-y-1">
              <span className="text-[12px] font-medium">
                {t("api.save.newCollectionLabel")}
              </span>
              <Input
                onChange={(event) => setNewCollectionName(event.target.value)}
                placeholder={t("api.save.newCollectionPlaceholder")}
                value={newCollectionName}
              />
            </label>
          ) : (
            <label className="block space-y-1">
              <span className="text-[12px] font-medium">
                {t("api.save.subfolder")}
              </span>
              <Input
                onChange={(event) => setSubfolder(event.target.value)}
                placeholder={t("api.save.subfolderPlaceholder")}
                value={subfolder}
              />
            </label>
          )}
        </DialogBody>
        <DialogFooter>
          <Button onClick={onCancel} type="button" variant="ghost">
            {t("api.save.cancel")}
          </Button>
          <Button disabled={!canSave} onClick={handleSave} type="button">
            {saving ? t("api.save.saving") : t("api.save.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
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
