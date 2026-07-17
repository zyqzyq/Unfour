import type { ReactNode } from "react";
import type { ApiSavedRequest } from "@unfour/command-client";
import type { TreeViewItem } from "@unfour/ui";
import { methodBadgeLabel, methodToneClass } from "../model/request-tabs";
import {
  RequestActionMenu,
  RequestContextMenu,
  type RequestTreeActionContext,
} from "./ApiRequestTreeActions";

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

function MethodMeta({ method }: { method: string }) {
  return (
    <span
      className={`w-9 shrink-0 text-left text-[10px] font-bold uppercase tabular-nums ${methodToneClass(method)}`}
    >
      {methodBadgeLabel(method)}
    </span>
  );
}

export function SidebarEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
      {children}
    </div>
  );
}

