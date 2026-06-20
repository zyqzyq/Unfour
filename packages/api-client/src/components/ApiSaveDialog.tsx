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
import type { ApiCollection, ApiSavedRequest } from "@unfour/command-client";
import { groupRequestsByCollection, type FolderNode } from "../request-utils";

const UNFILED_KEY = "~unfiled";

export type SaveIdentity = {
  collectionId: string | null;
  createCollectionName?: string;
  folderPath: string;
  name: string;
};

type Target = { collectionId: string | null; folderPath: string };

function targetId(collectionId: string | null, folderPath: string) {
  return `pick:${collectionId ?? UNFILED_KEY}:${folderPath}`;
}

export function ApiSaveDialog({
  collections,
  defaultCollectionId,
  defaultFolder,
  defaultName,
  onCancel,
  onSave,
  open,
  savedRequests,
  saving,
}: {
  collections: ApiCollection[];
  defaultCollectionId: string | null;
  defaultFolder: string;
  defaultName: string;
  onCancel: () => void;
  onSave: (identity: SaveIdentity) => void;
  open: boolean;
  savedRequests: ApiSavedRequest[];
  saving: boolean;
}) {
  const { t } = useI18n();

  // Build the location tree (collections + nested folders) and a lookup from
  // node id -> save target. Requests themselves are not shown; this is a
  // location picker.
  const targets = new Map<string, Target>();
  const registerTarget = (collectionId: string | null, folderPath: string) => {
    const id = targetId(collectionId, folderPath);
    targets.set(id, { collectionId, folderPath });
    return id;
  };

  function folderItem(node: FolderNode, collectionId: string | null): TreeViewItem {
    return {
      id: registerTarget(collectionId, node.path),
      icon: <Folder size={13} />,
      label: node.name,
      children: node.folders.map((child) => folderItem(child, collectionId)),
    };
  }

  const groups = groupRequestsByCollection(savedRequests, collections, "");
  const unfiledGroup = groups.find((group) => group.id === null);
  const items: TreeViewItem[] = [
    {
      id: registerTarget(null, ""),
      icon: <FolderOpen size={13} />,
      label: t("api.save.unfiled"),
      children: (unfiledGroup?.tree.folders ?? []).map((folder) =>
        folderItem(folder, null),
      ),
    },
    ...groups
      .filter((group) => group.id !== null)
      .map<TreeViewItem>((group) => ({
        id: registerTarget(group.id, ""),
        icon: <FolderOpen size={13} />,
        label: group.name,
        children: group.tree.folders.map((folder) =>
          folderItem(folder, group.id),
        ),
      })),
  ];

  const defaultId = targetId(defaultCollectionId, defaultFolder);
  const autoSelectedId = targets.has(defaultId) ? defaultId : targetId(null, "");

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
        folderPath: sub,
        name: name.trim(),
      });
      return;
    }
    const target = targets.get(effectiveSelectedId) ?? {
      collectionId: null,
      folderPath: "",
    };
    const folderPath = sub
      ? target.folderPath
        ? `${target.folderPath}/${sub}`
        : sub
      : target.folderPath;
    onSave({ collectionId: target.collectionId, folderPath, name: name.trim() });
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
