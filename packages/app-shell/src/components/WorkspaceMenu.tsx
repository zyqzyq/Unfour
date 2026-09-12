import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { ChevronDown, Folder, Pencil, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Workspace, WorkspaceMcpPolicy } from "@unfour/command-client";
import { Badge, Button, cn, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import type {
  DesktopAppExtensionContext,
  DesktopAppWorkspaceAction,
  DesktopAppWorkspaceActionContext,
  DesktopAppWorkspaceActionsProvider,
  DesktopAppWorkspaceDecoration,
  DesktopAppWorkspaceMenuFooterAction,
} from "../extensions";
import { WorkspaceDialogs } from "./WorkspaceDialogs";

export function WorkspaceMenu({
  activeWorkspace,
  className,
  decoration: Decoration,
  extensionContext,
  onActivateWorkspace,
  workspaceActionProvider,
  workspaceActions = [],
  workspaceMenuFooterActions = [],
  workspaces,
}: {
  activeWorkspace?: Workspace;
  className?: string;
  decoration?: DesktopAppWorkspaceDecoration;
  extensionContext: DesktopAppExtensionContext;
  onActivateWorkspace: (workspaceId: string) => void;
  workspaceActionProvider?: DesktopAppWorkspaceActionsProvider;
  workspaceActions?: readonly DesktopAppWorkspaceAction[];
  workspaceMenuFooterActions?: readonly DesktopAppWorkspaceMenuFooterAction[];
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();
  const [createOpen, setCreateOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [environmentOpen, setEnvironmentOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [pendingActionId, setPendingActionId] = useState<string | null>(null);
  const providedWorkspaceActions = activeWorkspace
    ? (workspaceActionProvider?.(extensionContext, activeWorkspace) ?? [])
    : [];
  const activeWorkspaceActions = [...workspaceActions, ...providedWorkspaceActions];

  async function runWorkspaceAction(
    action: DesktopAppWorkspaceAction,
    context: DesktopAppWorkspaceActionContext,
  ) {
    setPendingActionId(action.id);
    try {
      await action.run(context);
    } catch (error) {
      handleError(error, { key: "feedback.command.actionFailed" });
    } finally {
      setPendingActionId(null);
    }
  }

  async function runFooterAction(action: DesktopAppWorkspaceMenuFooterAction) {
    setPendingActionId(action.id);
    try {
      await action.run(extensionContext);
    } catch (error) {
      handleError(error, { key: "feedback.command.actionFailed" });
    } finally {
      setPendingActionId(null);
    }
  }

  return (
    <>
      <DropdownMenu.Root>
        <WorkspaceMenuTrigger
          activeWorkspace={activeWorkspace}
          className={className}
          decoration={Decoration}
          extensionContext={extensionContext}
        />
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
              <WorkspaceMenuItem
                active={activeWorkspace?.id === workspace.id}
                decoration={Decoration}
                extensionContext={extensionContext}
                key={workspace.id}
                onSelect={() => onActivateWorkspace(workspace.id)}
                workspace={workspace}
              />
            ))}
            {workspaces.length === 0 && (
              <div className="px-2 py-4 text-center text-xs text-[var(--u-color-text-muted)]">
                {t("app.workspace.noneAvailable")}
              </div>
            )}
            <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
            <WorkspaceMenuCoreActions
              activeWorkspace={activeWorkspace}
              onCreate={() => setCreateOpen(true)}
              onDelete={() => setDeleteOpen(true)}
              onEnvironment={() => setEnvironmentOpen(true)}
              onRename={() => setRenameOpen(true)}
              workspaceCount={workspaces.length}
            />
            {activeWorkspace && activeWorkspaceActions.length > 0 && (
              <>
                <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
                {activeWorkspaceActions.map((action) => {
                  const context: DesktopAppWorkspaceActionContext = {
                    ...extensionContext,
                    workspace: activeWorkspace,
                  };
                  const actionDisabled = resolveDisabled(action, context);
                  const disabled = pendingActionId !== null || actionDisabled;
                  const disabledReason = actionDisabled
                    ? resolveDisabledReason(action, context)
                    : undefined;
                  return (
                    <DropdownMenu.Item
                      className="flex min-h-8 cursor-pointer items-center gap-2 rounded px-2 py-1 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50"
                      disabled={disabled}
                      key={action.id}
                      onSelect={() => void runWorkspaceAction(action, context)}
                    >
                      {action.icon}
                      <span className="min-w-0 flex-1">
                        <span className="block truncate">{action.label}</span>
                        {disabled && disabledReason && (
                          <span className="block text-[11px] text-[var(--u-color-text-soft)]">
                            {disabledReason}
                          </span>
                        )}
                      </span>
                    </DropdownMenu.Item>
                  );
                })}
              </>
            )}
            {workspaceMenuFooterActions.length > 0 && (
              <>
                <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
                {workspaceMenuFooterActions.map((action) => {
                  const actionDisabled =
                    typeof action.disabled === "function"
                      ? action.disabled(extensionContext)
                      : Boolean(action.disabled);
                  return (
                    <DropdownMenu.Item
                      className="flex min-h-8 cursor-pointer items-center gap-2 rounded px-2 py-1 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50"
                      disabled={pendingActionId !== null || actionDisabled}
                      key={action.id}
                      onSelect={() => void runFooterAction(action)}
                    >
                      {action.icon}
                      <span className="min-w-0 flex-1 truncate">{action.label}</span>
                    </DropdownMenu.Item>
                  );
                })}
              </>
            )}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <WorkspaceDialogs
        activeWorkspace={activeWorkspace}
        createOpen={createOpen}
        deleteOpen={deleteOpen}
        environmentOpen={environmentOpen}
        onCreateClose={() => setCreateOpen(false)}
        onDeleteClose={() => setDeleteOpen(false)}
        onEnvironmentClose={() => setEnvironmentOpen(false)}
        onRenameClose={() => setRenameOpen(false)}
        renameOpen={renameOpen}
        workspaces={workspaces}
      />
    </>
  );
}

export function WorkspaceMenuTrigger({
  activeWorkspace,
  className,
  decoration: Decoration,
  extensionContext,
}: {
  activeWorkspace?: Workspace;
  className?: string;
  decoration?: DesktopAppWorkspaceDecoration;
  extensionContext: DesktopAppExtensionContext;
}) {
  const { t } = useI18n();
  return (
    <DropdownMenu.Trigger asChild>
      <Button
        className={cn(
          "w-[220px] justify-start gap-1 border-transparent bg-[var(--u-color-surface)] px-2 font-semibold shadow-none hover:bg-[var(--u-color-surface-hover)]",
          className,
        )}
        size="sm"
        title={activeWorkspace?.name ?? t("app.workspace.none")}
        type="button"
        variant="outline"
      >
        <span className="h-4 w-4 shrink-0 rounded-[5px] bg-[linear-gradient(135deg,var(--u-color-primary),var(--u-color-primary-hover))]" />
        <span className="min-w-0 truncate" title={activeWorkspace?.name}>
          {activeWorkspace?.name ?? t("app.workspace.none")}
        </span>
        {activeWorkspace && (
          <>
            <Badge
              className="shrink-0 px-1.5 leading-4"
              tone={environmentTone(activeWorkspace.environmentType)}
            >
              {environmentBadge(activeWorkspace.environmentType)}
            </Badge>
            {Decoration && (
              <Decoration
                {...extensionContext}
                active
                placement="trigger"
                workspace={activeWorkspace}
              />
            )}
          </>
        )}
        <ChevronDown className="ml-auto shrink-0 text-[var(--u-color-text-muted)]" size={14} />
      </Button>
    </DropdownMenu.Trigger>
  );
}

function WorkspaceMenuItem({
  active,
  decoration: Decoration,
  extensionContext,
  onSelect,
  workspace,
}: {
  active: boolean;
  decoration?: DesktopAppWorkspaceDecoration;
  extensionContext: DesktopAppExtensionContext;
  onSelect: () => void;
  workspace: Workspace;
}) {
  const { t } = useI18n();
  return (
    <DropdownMenu.Item
      className={cn(
        "flex min-h-8 cursor-pointer items-center gap-2 rounded px-2 py-1.5 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)]",
        active && "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]",
      )}
      onSelect={onSelect}
    >
      <Folder className="shrink-0" size={14} />
      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-center gap-1.5">
          <span className="min-w-0 truncate" title={workspace.name}>
            {workspace.name}
          </span>
          <Badge
            className="shrink-0 px-1.5 leading-4"
            tone={environmentTone(workspace.environmentType)}
          >
            {environmentBadge(workspace.environmentType)}
          </Badge>
          {workspace.isDefault && <Badge tone="teal">{t("app.workspace.defaultBadge")}</Badge>}
          {Decoration && (
            <Decoration
              {...extensionContext}
              active={active}
              placement="listItem"
              workspace={workspace}
            />
          )}
        </span>
        <span className="block truncate text-xs text-[var(--u-color-text-muted)]">
          {t(policySummaryKey(workspace))}
        </span>
      </span>
    </DropdownMenu.Item>
  );
}

function WorkspaceMenuCoreActions({
  activeWorkspace,
  onCreate,
  onDelete,
  onEnvironment,
  onRename,
  workspaceCount,
}: {
  activeWorkspace?: Workspace;
  onCreate: () => void;
  onDelete: () => void;
  onEnvironment: () => void;
  onRename: () => void;
  workspaceCount: number;
}) {
  const { t } = useI18n();
  const itemClass =
    "flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] disabled:pointer-events-none disabled:opacity-50";
  return (
    <>
      <DropdownMenu.Item className={itemClass} onSelect={onCreate}>
        <Plus size={14} />
        {t("app.workspace.new")}
      </DropdownMenu.Item>
      <DropdownMenu.Item className={itemClass} disabled={!activeWorkspace} onSelect={onRename}>
        <Pencil size={14} />
        {t("app.workspace.renameCurrent")}
      </DropdownMenu.Item>
      <DropdownMenu.Item
        className={itemClass}
        disabled={!activeWorkspace}
        onSelect={onEnvironment}
      >
        <ShieldCheck size={14} />
        {t("app.workspace.changeEnvironment")}
      </DropdownMenu.Item>
      <DropdownMenu.Item
        className={cn(
          itemClass,
          "text-[var(--u-color-danger-text)] hover:bg-[var(--u-color-danger-soft)] focus:bg-[var(--u-color-danger-soft)]",
        )}
        disabled={!activeWorkspace || activeWorkspace.isDefault || workspaceCount <= 1}
        onSelect={onDelete}
      >
        <Trash2 size={14} />
        {t("app.workspace.deleteCurrent")}
      </DropdownMenu.Item>
    </>
  );
}

function resolveDisabled(
  action: DesktopAppWorkspaceAction,
  context: DesktopAppWorkspaceActionContext,
) {
  return typeof action.disabled === "function"
    ? action.disabled(context)
    : Boolean(action.disabled);
}

function resolveDisabledReason(
  action: DesktopAppWorkspaceAction,
  context: DesktopAppWorkspaceActionContext,
) {
  return typeof action.disabledReason === "function"
    ? action.disabledReason(context)
    : action.disabledReason;
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
  if (workspace.mcpPolicy !== "auto") return workspace.mcpPolicy;
  if (workspace.environmentType === "prod") return "read_only";
  if (workspace.environmentType === "test") return "guarded";
  return "full_access";
}
