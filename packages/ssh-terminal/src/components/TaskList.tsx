import type { SshTask } from "@unfour/command-client";
import { useMemo, useState } from "react";
import {
  Button,
  ContextMenuItem,
  ContextMenuSeparator,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  ErrorState,
  IconButton,
  Input,
  LoadingState,
  SidebarSection,
  TreeView,
  type TreeViewDropPosition,
  type TreeViewItem,
  useI18n,
} from "@unfour/ui";
import { ArrowUpDown, Check, Play, Plus, Workflow } from "lucide-react";
import {
  reorderTaskIds,
  sortTasksForView,
  type TaskSortMode,
} from "../model/task-list-order";
import { SshSidebarModeSwitcher } from "./SshSidebarModeSwitcher";

export function TaskList({
  failed,
  loading,
  onDelete,
  onDuplicate,
  onExample,
  onNew,
  onOpenConnections,
  onReorder,
  onRun,
  onRetry,
  onSelect,
  reordering,
  retrying,
  selectedTaskId,
  tasks,
}: {
  failed: boolean;
  loading: boolean;
  onDelete: (task: SshTask) => void;
  onDuplicate: (task: SshTask) => void;
  onExample: () => void;
  onNew: () => void;
  onOpenConnections: () => void;
  onReorder: (taskIds: string[]) => void;
  onRun: (task: SshTask) => void;
  onRetry: () => void;
  onSelect: (taskId: string) => void;
  reordering: boolean;
  retrying: boolean;
  selectedTaskId: string | null;
  tasks: SshTask[];
}) {
  const { t } = useI18n();
  const [filter, setFilter] = useState("");
  const [sortMode, setSortMode] = useState<TaskSortMode>("manual");
  const manualTasks = useMemo(() => sortTasksForView(tasks, "manual"), [tasks]);
  const filtered = useMemo(() => {
    const query = filter.trim().toLowerCase();
    const matches = query
      ? manualTasks.filter(
          (task) =>
            task.name.toLowerCase().includes(query) ||
            task.description.toLowerCase().includes(query),
        )
      : manualTasks;
    return sortTasksForView(matches, sortMode);
  }, [filter, manualTasks, sortMode]);
  const reorderEnabled = sortMode === "manual" && !filter.trim() && !reordering;
  const manualTaskIds = useMemo(() => manualTasks.map((task) => task.id), [manualTasks]);
  const items: TreeViewItem[] = filtered.map((task) => {
    const index = manualTaskIds.indexOf(task.id);
    return {
      id: taskItemId(task.id),
      label: task.name,
      title: task.description || t("ssh.tasks.list.noDescription"),
      actions: (
        <IconButton
          disableTooltip
          label={t("ssh.tasks.actions.run")}
          onClick={(event) => {
            event.stopPropagation();
            onRun(task);
          }}
          size="compact"
        >
          <Play size={12} />
        </IconButton>
      ),
      contextMenu: (
        <>
          <ContextMenuItem onSelect={() => onRun(task)}>
            {t("ssh.tasks.actions.run")}
          </ContextMenuItem>
          <ContextMenuItem onSelect={() => onSelect(task.id)}>
            {t("ssh.tasks.actions.open")}
          </ContextMenuItem>
          <ContextMenuItem onSelect={() => onDuplicate(task)}>
            {t("ssh.tasks.actions.duplicate")}
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem
            disabled={!reorderEnabled || index <= 0}
            onSelect={() => moveTask(task.id, -1)}
          >
            {t("ssh.tasks.actions.moveTaskUp")}
          </ContextMenuItem>
          <ContextMenuItem
            disabled={!reorderEnabled || index >= manualTasks.length - 1}
            onSelect={() => moveTask(task.id, 1)}
          >
            {t("ssh.tasks.actions.moveTaskDown")}
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem onSelect={() => onDelete(task)} tone="danger">
            {t("ssh.tasks.actions.delete")}
          </ContextMenuItem>
        </>
      ),
    };
  });

  function moveTask(taskId: string, direction: -1 | 1) {
    const index = manualTaskIds.indexOf(taskId);
    const targetId = manualTaskIds[index + direction];
    if (!reorderEnabled || index < 0 || !targetId) return;
    onReorder(
      reorderTaskIds(
        manualTaskIds,
        taskId,
        targetId,
        direction < 0 ? "before" : "after",
      ),
    );
  }

  function dropTask(source: TreeViewItem, target: TreeViewItem, position: TreeViewDropPosition) {
    if (!reorderEnabled || position === "inside") return;
    onReorder(
      reorderTaskIds(
        manualTaskIds,
        taskIdFromItem(source),
        taskIdFromItem(target),
        position,
      ),
    );
  }

  return (
    <SidebarSection className="flex h-full min-h-0 flex-col">
      <div className="flex h-7 shrink-0 items-center justify-between gap-2 px-1">
        <SshSidebarModeSwitcher
          activeMode="tasks"
          onChange={(mode) => mode === "connections" && onOpenConnections()}
        />
        <div className="flex items-center gap-0.5">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <IconButton label={t("ssh.tasks.list.sortLabel")} size="compact">
                <ArrowUpDown size={14} />
              </IconButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {(["manual", "name", "updated"] as const).map((mode) => (
                <DropdownMenuItem key={mode} onSelect={() => setSortMode(mode)}>
                  <Check className={sortMode === mode ? "opacity-100" : "opacity-0"} size={13} />
                  {t(`ssh.tasks.list.sort.${mode}`)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <IconButton
            label={t("ssh.tasks.actions.new")}
            onClick={() => onNew()}
            size="compact"
          >
            <Plus size={14} />
          </IconButton>
        </div>
      </div>
      {failed && (
        <ErrorState className={tasks.length ? "min-h-0 shrink-0 justify-start rounded-none border-x-0 border-t-0 px-2 py-0.5 text-left [&>svg]:shrink-0 [&>span]:flex [&>span]:flex-1 [&>span]:items-center [&>span]:gap-2" : undefined}>
          <span className={tasks.length ? "min-w-0 flex-1 truncate" : undefined} role="alert" title={t("ssh.tasks.list.loadFailed")}>
            {t("ssh.tasks.list.loadFailed")}
          </span>{" "}
          <Button disabled={retrying} onClick={onRetry} size="sm" variant="secondary">
            {t("ssh.tasks.actions.retry")}
          </Button>
        </ErrorState>
      )}
      {loading ? (
        <LoadingState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.tasks.list.loading")}
        </LoadingState>
      ) : tasks.length ? (
        <>
          <div className="px-1 pb-1">
            <Input
              aria-label={t("ssh.tasks.list.filter")}
              className="h-7"
              onChange={(event) => setFilter(event.target.value)}
              placeholder={t("ssh.tasks.list.filterPlaceholder")}
              value={filter}
            />
          </div>
          {filtered.length ? (
            <div className="min-h-0 flex-1 overflow-y-auto py-1">
              <TreeView
                canDrag={() => reorderEnabled}
                canDrop={(_source, _target, position) =>
                  reorderEnabled && position !== "inside"
                }
                items={items}
                onActivate={(item) => onSelect(taskIdFromItem(item))}
                onDrop={({ position, source, target }) =>
                  dropTask(source, target, position)
                }
                onSelect={(item) => onSelect(taskIdFromItem(item))}
                selectedId={selectedTaskId ? taskItemId(selectedTaskId) : null}
              />
            </div>
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center px-3 text-center text-[12px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.list.filterEmpty")}
            </div>
          )}
        </>
      ) : !failed && (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 p-4 text-center">
          <Workflow className="text-[var(--u-color-text-soft)]" size={24} />
          <div>
            <p className="text-[13px] font-medium text-[var(--u-color-text)]">
              {t("ssh.tasks.list.emptyTitle")}
            </p>
            <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.list.emptyDescription")}
            </p>
          </div>
          <Button onClick={() => onNew()} size="sm">
            {t("ssh.tasks.actions.new")}
          </Button>
          <Button onClick={onExample} size="sm" variant="secondary">
            {t("ssh.tasks.actions.dockerExample")}
          </Button>
        </div>
      )}
    </SidebarSection>
  );
}

function taskItemId(taskId: string) {
  return `task:${taskId}`;
}

function taskIdFromItem(item: TreeViewItem) {
  return item.id.startsWith("task:") ? item.id.slice("task:".length) : item.id;
}
