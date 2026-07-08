import { Clock3 } from "lucide-react";
import {
  ContextMenuItem,
  TreeView,
  type TreeViewItem,
  useI18n,
} from "@unfour/ui";
import type { ApiHistoryItem } from "@unfour/command-client";
import { groupApiHistory } from "../model/request-tabs";
import type { ApiOpenIntent } from "../model/types";

export function ApiHistoryTree({
  items,
  onOpenIntent,
}: {
  items: ApiHistoryItem[];
  onOpenIntent: (intent: ApiOpenIntent) => void;
}) {
  const { t } = useI18n();
  const treeItems: TreeViewItem[] = groupApiHistory(items).map((group) => ({
    id: group.id,
    icon: <Clock3 size={13} />,
    label: group.label,
    children: group.items.map((item) => ({
      id: `history-item:${item.id}`,
      label: item.url,
      title: `${item.method} ${item.url}`,
      meta: <HistoryMeta item={item} />,
      contextMenu: (
        <>
          <ContextMenuItem
            onSelect={() =>
              onOpenIntent({ historyId: item.id, kind: "history", nonce: Date.now() })
            }
          >
            {t("api.history.open")}
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() =>
              onOpenIntent({
                action: "save",
                historyId: item.id,
                kind: "history",
                nonce: Date.now(),
              })
            }
          >
            {t("api.history.saveAsRequest")}
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => void navigator.clipboard?.writeText(item.url)}
          >
            {t("api.request.copyUrl")}
          </ContextMenuItem>
        </>
      ),
    })),
  }));
  return (
    <TreeView
      defaultExpandedIds={treeItems.slice(0, 2).map((item) => item.id)}
      items={treeItems}
      onSelect={(item) => {
        if (item.id.startsWith("history-item:")) {
          onOpenIntent({
            historyId: item.id.slice("history-item:".length),
            kind: "history",
            nonce: Date.now(),
          });
        }
      }}
    />
  );
}

function HistoryMeta({ item }: { item: ApiHistoryItem }) {
  return (
    <span className="flex min-w-0 items-center gap-1 text-[10px] text-[var(--u-color-text-soft)]">
      <span className="rounded-[var(--u-radius-sm)] bg-[var(--u-color-surface-muted)] px-1 font-semibold uppercase text-[var(--u-color-text-muted)]">
        {item.method}
      </span>
      {item.status !== null && <span>{item.status}</span>}
      {item.durationMs !== null && <span>{item.durationMs}ms</span>}
      <span>{formatHistoryTime(item.createdAt)}</span>
    </span>
  );
}

function formatHistoryTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
