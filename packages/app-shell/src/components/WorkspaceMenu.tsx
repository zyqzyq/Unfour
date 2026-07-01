import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ChevronDown,
  Folder,
  Pencil,
  Plus,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import type { Workspace, WorkspaceMcpPolicy } from "@unfour/command-client";
import { Badge, Button, cn, useI18n } from "@unfour/ui";
import { WorkspaceDialogs } from "./WorkspaceDialogs";

export function WorkspaceMenu({
  activeWorkspace,
  className,
  onActivateWorkspace,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  className?: string;
  onActivateWorkspace: (workspaceId: string) => void;
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const [createOpen, setCreateOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [environmentOpen, setEnvironmentOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  return (
    <>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button
            className={cn(
              "w-[220px] justify-start gap-1 border-transparent bg-[var(--u-color-surface)] px-2 font-semibold shadow-none hover:bg-[var(--u-color-surface-hover)]",
              className,
            )}
            size="sm"
            type="button"
            variant="outline"
          >
            <span className="h-4 w-4 shrink-0 rounded-[5px] bg-[linear-gradient(135deg,var(--u-color-primary),var(--u-color-primary-hover))]" />
            <span className="min-w-0  truncate">
              {activeWorkspace?.name ?? t("app.workspace.none")}
            </span>
            {activeWorkspace && (
              <Badge className="shrink-0 px-1.5 leading-4" tone={environmentTone(activeWorkspace.environmentType)}>
                {environmentBadge(activeWorkspace.environmentType)}
              </Badge>
            )}
            <ChevronDown className="ml-auto shrink-0 text-[var(--u-color-text-muted)]" size={14} />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="start"
            className="z-50 w-72 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1 text-sm text-[var(--u-color-text)] shadow-xl"
            sideOffset={6}
          >
            <DropdownMenu.Label className="px-2 py-1.5 text-xs font-semibold uppercase text-[var(--u-color-text-muted)]">
              {t("app.workspace.label")}
            </DropdownMenu.Label>
            {workspaces.map((workspace) => (
              <DropdownMenu.Item
                className={cn(
                  "flex min-h-8 cursor-pointer items-center gap-2 rounded px-2 py-1.5 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)]",
                  activeWorkspace?.id === workspace.id && "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]",
                )}
                key={workspace.id}
                onSelect={() => onActivateWorkspace(workspace.id)}
              >
                <Folder className="shrink-0" size={14} />
                <span className="min-w-0 flex-1">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="min-w-0 truncate">{workspace.name}</span>
                    <Badge className="shrink-0 px-1.5 leading-4" tone={environmentTone(workspace.environmentType)}>
                      {environmentBadge(workspace.environmentType)}
                    </Badge>
                    {workspace.isDefault && <Badge tone="teal">{t("app.workspace.defaultBadge")}</Badge>}
                  </span>
                  <span className="block truncate text-xs text-[var(--u-color-text-muted)]">
                    {t(policySummaryKey(workspace))}
                  </span>
                </span>
              </DropdownMenu.Item>
            ))}
            {workspaces.length === 0 && (
              <div className="px-2 py-4 text-center text-xs text-[var(--u-color-text-muted)]">
                {t("app.workspace.noneAvailable")}
              </div>
            )}
            <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)]"
              onSelect={() => setCreateOpen(true)}
            >
              <Plus size={14} />
              {t("app.workspace.new")}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] disabled:pointer-events-none disabled:opacity-50"
              disabled={!activeWorkspace}
              onSelect={() => setRenameOpen(true)}
            >
              <Pencil size={14} />
              {t("app.workspace.renameCurrent")}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] disabled:pointer-events-none disabled:opacity-50"
              disabled={!activeWorkspace}
              onSelect={() => setEnvironmentOpen(true)}
            >
              <ShieldCheck size={14} />
              {t("app.workspace.changeEnvironment")}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 text-[var(--u-color-danger-text)] outline-none hover:bg-[var(--u-color-danger-soft)] focus:bg-[var(--u-color-danger-soft)] disabled:pointer-events-none disabled:opacity-50"
              disabled={!activeWorkspace || activeWorkspace.isDefault || workspaces.length <= 1}
              onSelect={() => setDeleteOpen(true)}
            >
              <Trash2 size={14} />
              {t("app.workspace.deleteCurrent")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <WorkspaceDialogs
        activeWorkspace={activeWorkspace}
        createOpen={createOpen}
        deleteOpen={deleteOpen}
        onCreateClose={() => setCreateOpen(false)}
        onDeleteClose={() => setDeleteOpen(false)}
        onEnvironmentClose={() => setEnvironmentOpen(false)}
        onRenameClose={() => setRenameOpen(false)}
        environmentOpen={environmentOpen}
        renameOpen={renameOpen}
        workspaces={workspaces}
      />
    </>
  );
}

function environmentBadge(environmentType: Workspace["environmentType"]) {
  return environmentType.toUpperCase();
}

function environmentTone(environmentType: Workspace["environmentType"]): "green" | "amber" | "red" {
  if (environmentType === "prod") return "red";
  if (environmentType === "test") return "amber";
  return "green";
}

function policySummaryKey(workspace: Workspace) {
  switch (resolveMcpPolicy(workspace)) {
    case "disabled":
      return "app.workspace.mcp.disabled";
    case "read_only":
      return "app.workspace.mcp.readOnly";
    case "guarded":
      return "app.workspace.mcp.guarded";
    case "full_access":
      return "app.workspace.mcp.fullAccess";
    default:
      return "app.workspace.mcp.guarded";
  }
}

function resolveMcpPolicy(workspace: Workspace): Exclude<WorkspaceMcpPolicy, "auto"> {
  if (workspace.mcpPolicy !== "auto") {
    return workspace.mcpPolicy;
  }
  if (workspace.environmentType === "prod") {
    return "read_only";
  }
  if (workspace.environmentType === "test") {
    return "guarded";
  }
  return "full_access";
}
