import type * as React from "react";
import { Braces, Folder, Send } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { SidebarRow, SidebarSection } from "@unfour/ui";
import { listSavedApiRequests } from "@unfour/command-client";
import { groupSavedRequests } from "../request-utils";
import type { ApiResourceGroup } from "../model/types";

export function ApiCollectionTree({
  active,
  collapsed,
  onOpenClient,
  onSelectRequest,
  selectedId,
  workspaceId,
}: {
  active: boolean;
  collapsed: boolean;
  onOpenClient: () => void;
  onSelectRequest: (requestId: string) => void;
  selectedId: string | null;
  workspaceId: string;
}) {
  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const groups = groupSavedRequests(savedQuery.data ?? []);

  return (
    <div className="space-y-3">
      <SidebarSection title={collapsed ? undefined : "Collections"}>
        <div className="space-y-1">
          <SidebarRow active={active && (collapsed || !selectedId)} onClick={onOpenClient}>
            <Send size={14} />
            {!collapsed && <span className="truncate">REST Client</span>}
          </SidebarRow>
          {!collapsed && (
            <CollectionGroups
              groups={groups}
              onSelectRequest={onSelectRequest}
              selectedId={selectedId}
            />
          )}
          {!collapsed && groups.length === 0 && <SidebarEmpty>No saved requests</SidebarEmpty>}
        </div>
      </SidebarSection>
      <SidebarSection title={collapsed ? undefined : "Environments"}>
        {!collapsed && <SidebarEmpty>No environment selected</SidebarEmpty>}
      </SidebarSection>
      <SidebarSection title={collapsed ? undefined : "History"}>
        {!collapsed && <SidebarEmpty>Send a request to build history</SidebarEmpty>}
      </SidebarSection>
    </div>
  );
}

function CollectionGroups({
  groups,
  onSelectRequest,
  selectedId,
}: {
  groups: ApiResourceGroup[];
  onSelectRequest: (requestId: string) => void;
  selectedId: string | null;
}) {
  if (groups.length === 0) {
    return null;
  }

  return (
    <div className="mt-1 space-y-2 border-l border-[var(--u-color-border)] pl-2">
      {groups.map((group) => (
        <div key={group.folder}>
          <div className="flex h-6 items-center gap-1.5 px-2 text-[11px] font-medium text-[var(--u-color-text-soft)]">
            <Folder size={12} />
            <span className="min-w-0 truncate">{group.folder}</span>
          </div>
          <div className="space-y-1">
            {group.items.map((request) => (
              <SidebarRow
                active={selectedId === request.id}
                key={request.id}
                onClick={() => onSelectRequest(request.id)}
              >
                <span className="flex h-4 w-4 shrink-0 items-center justify-center">
                  <Braces size={13} />
                </span>
                <span className="min-w-0 flex-1 truncate">{request.name}</span>
                <span className="shrink-0 rounded-[var(--u-radius-sm)] bg-[var(--u-color-surface-muted)] px-1.5 text-[10px] font-medium uppercase leading-5 text-[var(--u-color-text-soft)]">
                  {request.method}
                </span>
              </SidebarRow>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function SidebarEmpty({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-[var(--u-radius-sm)] border border-dashed border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 py-2 text-[12px] text-[var(--u-color-text-muted)]">
      {children}
    </div>
  );
}
