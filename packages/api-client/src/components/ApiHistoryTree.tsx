import { Clock3 } from "lucide-react";
import {
  cn,
  ContextMenuItem,
  TreeView,
  type TreeViewItem,
  useI18n,
} from "@unfour/ui";
import type { ApiHistoryItem } from "@unfour/command-client";
import {
  groupApiHistory,
  methodBadgeLabel,
  methodBadgeToneClass,
} from "../model/request-tabs";
import {
  apiHistoryPrimaryLabel,
  apiHistoryRequestName,
  apiHistoryTooltip,
} from "../model/api-history-display";
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
      label: <HistoryLabel item={item} />,
      title: apiHistoryTooltip(item),
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
  const time = formatHistoryTime(item.createdAt);
  return (
    <span className="flex min-w-0 items-center gap-1 text-[10px] text-[var(--u-color-text-soft)]">
      {item.status !== null && (
        <span
          className={cn(
            "font-mono font-semibold tabular-nums",
            statusToneClass(item.status),
          )}
        >
          {item.status}
        </span>
      )}
      {item.durationMs !== null && <span>{item.durationMs}ms</span>}
      {time && <span>{time}</span>}
    </span>
  );
}

function HistoryLabel({ item }: { item: ApiHistoryItem }) {
  const hasName = Boolean(apiHistoryRequestName(item));
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span
        className={cn(
          "shrink-0 rounded-[var(--u-radius-sm)] px-1 font-mono text-[10px] font-bold uppercase",
          methodBadgeToneClass(item.method),
        )}
      >
        {methodBadgeLabel(item.method)}
      </span>
      <span
        className={cn(
          "min-w-0 flex-1 truncate",
          hasName
            ? "font-medium text-[var(--u-color-text)]"
            : "font-mono text-[var(--u-color-text-muted)]",
        )}
      >
        {apiHistoryPrimaryLabel(item)}
      </span>
    </span>
  );
}

function statusToneClass(status: number): string {
  if (status >= 500) {
    return "text-[var(--u-color-danger-text)]";
  }
  if (status >= 400) {
    return "text-[var(--u-color-warning-text)]";
  }
  if (status >= 200 && status < 300) {
    return "text-[var(--u-color-success)]";
  }
  return "text-[var(--u-color-text-muted)]";
}

function formatHistoryTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
