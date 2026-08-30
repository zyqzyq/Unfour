import type { ApiSavedRequest } from "@unfour/command-client";
import type { TreeViewItem } from "@unfour/ui";
import { MethodMeta } from "./ApiTreeLabels";
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
