import type { SshTask } from "@unfour/command-client";
import { Button, IconButton, LoadingState, useI18n } from "@unfour/ui";
import { Copy, Play, Plus, Trash2, Workflow } from "lucide-react";
import { SshSidebarModeSwitcher } from "./SshSidebarModeSwitcher";

export function TaskList({
  loading,
  onDelete,
  onDuplicate,
  onExample,
  onNew,
  onOpenConnections,
  onRun,
  onSelect,
  selectedTaskId,
  tasks,
}: {
  loading: boolean;
  onDelete: (task: SshTask) => void;
  onDuplicate: (task: SshTask) => void;
  onExample: () => void;
  onNew: () => void;
  onOpenConnections: () => void;
  onRun: (task: SshTask) => void;
  onSelect: (taskId: string) => void;
  selectedTaskId: string | null;
  tasks: SshTask[];
}) {
  const { t } = useI18n();
  return (
    <aside className="flex h-full min-h-0 w-full min-w-0 flex-col bg-[var(--u-color-surface)]">
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <SshSidebarModeSwitcher
          activeMode="tasks"
          onChange={(mode) => mode === "connections" && onOpenConnections()}
        />
        <IconButton
          label={t("ssh.tasks.actions.new")}
          onClick={() => onNew()}
          size="compact"
        >
          <Plus size={14} />
        </IconButton>
      </div>
      {loading ? (
        <LoadingState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.tasks.list.loading")}
        </LoadingState>
      ) : tasks.length ? (
        <div className="min-h-0 flex-1 overflow-y-auto py-1">
          {tasks.map((task) => {
            const selected = selectedTaskId === task.id;
            return (
              <div
                className={`group flex min-h-9 items-center gap-1 rounded-[var(--u-radius-sm)] px-1.5 transition-colors ${
                  selected
                    ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
                    : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
                }`}
                key={task.id}
              >
                <button
                  className="min-w-0 flex-1 cursor-pointer py-0.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]"
                  onClick={() => onSelect(task.id)}
                  type="button"
                >
                  <span className="block truncate text-[12px] font-medium leading-4">{task.name}</span>
                  <span className="block truncate text-[10px] leading-3 text-[var(--u-color-text-soft)]">
                    {task.description || t("ssh.tasks.list.noDescription")}
                  </span>
                </button>
                <div className="flex shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                  <IconButton label={t("ssh.tasks.actions.run")} onClick={() => onRun(task)} size="compact">
                    <Play size={12} />
                  </IconButton>
                  <IconButton label={t("ssh.tasks.actions.duplicate")} onClick={() => onDuplicate(task)} size="compact">
                    <Copy size={12} />
                  </IconButton>
                  <IconButton className="text-[var(--u-color-danger)]" label={t("ssh.tasks.actions.delete")} onClick={() => onDelete(task)} size="compact">
                    <Trash2 size={12} />
                  </IconButton>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
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
    </aside>
  );
}
