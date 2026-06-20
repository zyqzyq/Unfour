import { useState } from "react";
import { ArrowLeft, Check, ChevronDown, Layers, Pencil, Plus, Trash2 } from "lucide-react";
import {
  Button,
  IconButton,
  Popover,
  PopoverContent,
  PopoverTrigger,
  cn,
  useI18n,
} from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import { useApiEnvironments } from "../hooks/useApiEnvironments";
import { EnvironmentEditor } from "./EnvironmentEditor";

/**
 * Request-bar environment control: a single button that both switches the
 * active environment and edits its variables inline, without leaving the
 * request context. Activation flows through the parent (shared with send
 * resolution); CRUD goes through `useApiEnvironments` (same query cache).
 */
export function EnvironmentControl({
  activeEnvironmentId,
  onSelectEnvironment,
  workspaceId,
}: {
  activeEnvironmentId: string | null;
  onSelectEnvironment: (environmentId: string | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const { activeEnvironment, createMut, deleteMut, environments, updateMut } =
    useApiEnvironments(workspaceId);

  const editing = environments.find((env) => env.id === editingId) ?? null;

  function close() {
    setOpen(false);
    setEditingId(null);
    updateMut.reset();
  }

  function handleSave(name: string, variables: KeyValue[]) {
    if (!editing) {
      return;
    }
    updateMut.mutate(
      { id: editing.id, name, variables },
      { onSuccess: () => setEditingId(null) },
    );
  }

  function handleCreate() {
    createMut.mutate(t("api.environment.defaultName"), {
      onSuccess: (environment) => setEditingId(environment.id),
    });
  }

  return (
    <Popover
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) {
          close();
        }
      }}
      open={open}
    >
      <PopoverTrigger asChild>
        <button
          aria-label={t("api.environment.active")}
          className="flex h-[var(--u-size-input)] max-w-[170px] shrink-0 items-center gap-1.5 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 text-[12px] text-[var(--u-color-text)] outline-none transition-colors hover:border-[var(--u-color-border-strong)] focus:border-[var(--u-color-focus)]"
          title={t("api.environment.active")}
          type="button"
        >
          <Layers className="shrink-0 text-[var(--u-color-text-muted)]" size={13} />
          <span className="min-w-0 flex-1 truncate text-left">
            {activeEnvironment?.name ?? t("api.environment.none")}
          </span>
          <ChevronDown className="shrink-0 text-[var(--u-color-text-soft)]" size={13} />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[280px]">
        {editing ? (
          <div className="flex flex-col">
            <div className="flex items-center gap-1 border-b border-[var(--u-color-border)] px-2 py-1.5">
              <IconButton
                label={t("api.environment.back")}
                onClick={() => setEditingId(null)}
                size="compact"
              >
                <ArrowLeft size={14} />
              </IconButton>
              <span className="min-w-0 flex-1 truncate text-[12px] font-semibold text-[var(--u-color-text)]">
                {editing.name}
              </span>
            </div>
            <div className="max-h-[60vh] overflow-y-auto p-2.5">
              <EnvironmentEditor
                environment={editing}
                onSave={handleSave}
                saveError={
                  updateMut.isError
                    ? updateMut.error instanceof Error
                      ? updateMut.error.message
                      : String(updateMut.error)
                    : null
                }
                saving={updateMut.isPending}
              />
            </div>
          </div>
        ) : (
          <div className="flex flex-col py-1">
            <EnvironmentRow
              active={activeEnvironmentId === null}
              label={t("api.environment.none")}
              muted
              onActivate={() => {
                onSelectEnvironment(null);
                close();
              }}
            />
            {environments.length > 0 && (
              <div className="my-1 border-t border-[var(--u-color-border)]" />
            )}
            {environments.map((environment) => (
              <EnvironmentRow
                active={activeEnvironmentId === environment.id}
                key={environment.id}
                label={environment.name}
                onActivate={() => {
                  onSelectEnvironment(environment.id);
                  close();
                }}
                onDelete={() => deleteMut.mutate(environment.id)}
                onEdit={() => setEditingId(environment.id)}
              />
            ))}
            <div className="mt-1 border-t border-[var(--u-color-border)] px-2 pt-1.5">
              <Button
                className="w-full justify-start"
                disabled={createMut.isPending}
                onClick={handleCreate}
                size="sm"
                type="button"
                variant="ghost"
              >
                <Plus size={13} />
                {t("api.environment.newEnvironment")}
              </Button>
            </div>
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

function EnvironmentRow({
  active,
  label,
  muted,
  onActivate,
  onDelete,
  onEdit,
}: {
  active: boolean;
  label: string;
  muted?: boolean;
  onActivate: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
}) {
  const { t } = useI18n();

  return (
    <div
      className={cn(
        "group flex items-center gap-1 px-1.5",
        active && "bg-[var(--u-color-primary-soft)]",
      )}
    >
      <button
        className="flex min-w-0 flex-1 items-center gap-2 rounded-[var(--u-radius-sm)] px-1.5 py-1.5 text-left hover:bg-[var(--u-color-surface-hover)]"
        onClick={onActivate}
        type="button"
      >
        <Check
          className={cn(
            "shrink-0",
            active ? "text-[var(--u-color-primary)]" : "text-transparent",
          )}
          size={14}
        />
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[12px]",
            muted
              ? "text-[var(--u-color-text-muted)]"
              : "font-medium text-[var(--u-color-text)]",
            active && "text-[var(--u-color-primary)]",
          )}
        >
          {label}
        </span>
      </button>
      {onEdit && (
        <button
          aria-label={t("api.environment.edit")}
          className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
          onClick={onEdit}
          title={t("api.environment.edit")}
          type="button"
        >
          <Pencil size={13} />
        </button>
      )}
      {onDelete && (
        <button
          aria-label={t("api.environment.delete")}
          className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)]"
          onClick={onDelete}
          title={t("api.environment.delete")}
          type="button"
        >
          <Trash2 size={13} />
        </button>
      )}
    </div>
  );
}
