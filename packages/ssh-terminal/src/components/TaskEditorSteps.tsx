import type {
  SshTaskStepInput,
  SshTaskStepType,
} from "@unfour/command-client";
import type { PointerEvent as ReactPointerEvent, ReactNode } from "react";
import {
  Badge,
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  Input,
  useI18n,
} from "@unfour/ui";
import { ChevronDown, ChevronRight, GripVertical, MoreHorizontal, Plus } from "lucide-react";
import { LocalPathField } from "./TaskPathFields";

export function StepRow({
  advancedOpen,
  dragOver,
  dragging,
  expanded,
  index,
  onConfigChange,
  onDragHandlePointerDown,
  onDuplicate,
  onMove,
  onRemove,
  onToggleAdvanced,
  onToggleExpand,
  onUpdate,
  step,
  stepCount,
}: {
  advancedOpen: boolean;
  dragOver: boolean;
  dragging: boolean;
  expanded: boolean;
  index: number;
  onConfigChange: (key: string, value: string | number | boolean) => void;
  onDragHandlePointerDown: (event: ReactPointerEvent<HTMLSpanElement>) => void;
  onDuplicate: () => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onToggleAdvanced: () => void;
  onToggleExpand: () => void;
  onUpdate: (patch: Partial<SshTaskStepInput>) => void;
  step: SshTaskStepInput;
  stepCount: number;
}) {
  const { t } = useI18n();
  const config = step.configJson as unknown as Record<string, string | number | boolean>;
  const preview = stepPreview(step);

  return (
    <article
      className={`overflow-hidden rounded-[var(--u-radius-lg)] border border-[var(--u-color-border)] bg-[var(--u-color-canvas)] shadow-[var(--u-shadow-sm)] ${
        step.enabled ? "" : "opacity-60"
      } ${expanded ? "border-[var(--u-color-border-strong)]" : ""} ${
        dragOver ? "border-[var(--u-color-primary)] ring-1 ring-[var(--u-color-primary)]" : ""
      } ${dragging ? "opacity-50" : ""}`}
      data-step-index={index}
    >
      <div
        className={`flex min-h-11 items-center gap-2 px-3 ${
          expanded
            ? "border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]"
            : "cursor-pointer hover:bg-[var(--u-color-surface-hover)]"
        }`}
        onClick={onToggleExpand}
        onKeyDown={(event) => {
          if (event.target !== event.currentTarget) return;
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onToggleExpand();
          }
        }}
        role="button"
        tabIndex={0}
      >
        <span
          aria-label={t("ssh.tasks.editor.dragStep")}
          className="grid h-6 w-5 shrink-0 cursor-grab touch-none select-none place-items-center text-[var(--u-color-text-soft)] hover:text-[var(--u-color-text)] active:cursor-grabbing"
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
          onPointerDown={(event) => {
            event.stopPropagation();
            onDragHandlePointerDown(event);
          }}
          role="button"
          tabIndex={0}
          title={t("ssh.tasks.editor.dragStep")}
        >
          <GripVertical aria-hidden className="pointer-events-none" size={13} />
        </span>
        <span className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] bg-[var(--u-color-primary-soft)] font-mono text-[11px] text-[var(--u-color-primary)]">
          {index + 1}
        </span>
        <Badge>{t(`ssh.tasks.stepTypes.${step.stepType}`)}</Badge>
        {expanded ? (
          <Input
            aria-label={t("ssh.tasks.editor.stepName")}
            className="h-6 min-w-0 flex-1"
            maxLength={128}
            onChange={(event) => onUpdate({ name: event.target.value })}
            onClick={(event) => event.stopPropagation()}
            value={step.name}
          />
        ) : (
          <>
            <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-[var(--u-color-text)]">
              {preview || t("ssh.tasks.editor.emptyStepPreview")}
            </span>
            {step.stepType === "command" && String(config.workingDirectory ?? "") ? (
              <span className="hidden max-w-[180px] truncate text-[11px] text-[var(--u-color-text-soft)] sm:inline">
                {String(config.workingDirectory)}
              </span>
            ) : null}
            {!step.enabled && (
              <span className="shrink-0 text-[10px] text-[var(--u-color-text-soft)]">
                {t("ssh.tasks.run.disabled")}
              </span>
            )}
          </>
        )}
        <StepActionsMenu
          enabled={step.enabled}
          index={index}
          onDuplicate={onDuplicate}
          onMove={onMove}
          onRemove={onRemove}
          onToggleEnabled={() => onUpdate({ enabled: !step.enabled })}
          stepCount={stepCount}
        />
      </div>

      {expanded && (
        <div className="flex flex-col gap-3 p-4" onClick={(event) => event.stopPropagation()}>
          {step.stepType === "command" ? (
            <>
              <div className="relative">
                <span className="pointer-events-none absolute left-2 top-2 font-mono text-[12px] text-[var(--u-color-terminal-muted)]">
                  $
                </span>
                <textarea
                  aria-label={t("ssh.tasks.editor.command")}
                  className="min-h-[96px] w-full resize-y rounded-[var(--u-radius-md)] border border-[var(--u-color-terminal-input-border)] bg-[var(--u-color-terminal-bg)] py-2 pl-6 pr-3 font-mono text-[12px] leading-5 text-[var(--u-color-terminal-text)] outline-none focus:border-[var(--u-color-terminal-cursor)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-terminal-cursor)_16%,transparent)]"
                  onChange={(event) => onConfigChange("command", event.target.value)}
                  placeholder={t("ssh.tasks.editor.commandPlaceholder")}
                  rows={3}
                  value={String(config.command ?? "")}
                />
              </div>
              <button
                className="flex w-fit cursor-pointer items-center gap-1 text-[11px] font-medium text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
                onClick={onToggleAdvanced}
                type="button"
              >
                {advancedOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                {t("ssh.tasks.editor.advanced")}
              </button>
              {advancedOpen && (
                <div className="flex flex-wrap items-center gap-2 rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 py-1.5">
                  <InlineField label={t("ssh.tasks.editor.workingDirectory")}>
                    <Input
                      aria-label={t("ssh.tasks.editor.workingDirectory")}
                      className="h-7 w-[220px]"
                      onChange={(event) =>
                        onConfigChange("workingDirectory", event.target.value)
                      }
                      value={String(config.workingDirectory ?? "")}
                    />
                  </InlineField>
                  <InlineField label={t("ssh.tasks.editor.timeoutSeconds")}>
                    <Input
                      aria-label={t("ssh.tasks.editor.timeoutSeconds")}
                      className="h-7 w-20"
                      max={3600}
                      min={1}
                      onChange={(event) =>
                        onConfigChange("timeoutSeconds", Number(event.target.value))
                      }
                      type="number"
                      value={Number(config.timeoutSeconds ?? 300)}
                    />
                  </InlineField>
                  <label className="flex h-7 cursor-pointer items-center gap-1.5 text-[11px] text-[var(--u-color-text-muted)]">
                    <input
                      checked={Boolean(config.continueOnError)}
                      onChange={(event) =>
                        onConfigChange("continueOnError", event.target.checked)
                      }
                      type="checkbox"
                    />
                    {t("ssh.tasks.editor.continueOnError")}
                  </label>
                </div>
              )}
            </>
          ) : (
            <>
              <div className="grid grid-cols-2 gap-2">
                <LocalPathField
                  label={t("ssh.tasks.editor.localPath")}
                  mode={step.stepType === "upload" ? "upload" : "download"}
                  onChange={(next) => onConfigChange("localPath", next)}
                  remotePath={String(config.remotePath ?? "")}
                  value={String(config.localPath ?? "")}
                />
                <Field label={t("ssh.tasks.editor.remotePath")}>
                  <Input
                    className="font-mono text-[12px]"
                    onChange={(event) => onConfigChange("remotePath", event.target.value)}
                    placeholder={t("ssh.tasks.editor.remotePathPlaceholder")}
                    value={String(config.remotePath ?? "")}
                  />
                </Field>
              </div>
              <button
                className="flex w-fit cursor-pointer items-center gap-1 text-[11px] font-medium text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
                onClick={onToggleAdvanced}
                type="button"
              >
                {advancedOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                {t("ssh.tasks.editor.advanced")}
              </button>
              {advancedOpen && (
                <label className="flex cursor-pointer items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
                  <input
                    checked={Boolean(config.overwrite)}
                    onChange={(event) => onConfigChange("overwrite", event.target.checked)}
                    type="checkbox"
                  />
                  {t("ssh.tasks.editor.overwrite")}
                </label>
              )}
            </>
          )}
        </div>
      )}
    </article>
  );
}

export function AddStepMenu({
  onAdd,
  variant = "outline",
}: {
  onAdd: (stepType: SshTaskStepType) => void;
  variant?: "outline" | "ghost";
}) {
  const { t } = useI18n();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" variant={variant}>
          <Plus size={13} />
          {t("ssh.tasks.actions.addStep")}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => onAdd("command")}>
          {t("ssh.tasks.actions.addCommand")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onAdd("upload")}>
          {t("ssh.tasks.actions.addUpload")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onAdd("download")}>
          {t("ssh.tasks.actions.addDownload")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function StepInsertSlot({ onAdd }: { onAdd: (stepType: SshTaskStepType) => void }) {
  const { t } = useI18n();
  return (
    <div className="group relative flex h-5 items-center justify-center">
      <div className="absolute inset-x-8 h-px bg-transparent transition-colors group-hover:bg-[var(--u-color-border)]" />
      <div className="relative z-10 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label={t("ssh.tasks.editor.insertStep")} size="compact">
              <Plus size={12} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="center">
            <DropdownMenuItem onSelect={() => onAdd("command")}>
              {t("ssh.tasks.actions.addCommand")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onAdd("upload")}>
              {t("ssh.tasks.actions.addUpload")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onAdd("download")}>
              {t("ssh.tasks.actions.addDownload")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}

function StepActionsMenu({
  enabled,
  index,
  onDuplicate,
  onMove,
  onRemove,
  onToggleEnabled,
  stepCount,
}: {
  enabled: boolean;
  index: number;
  onDuplicate: () => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onToggleEnabled: () => void;
  stepCount: number;
}) {
  const { t } = useI18n();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton
          label={t("ssh.tasks.editor.stepActions")}
          onClick={(event) => event.stopPropagation()}
          onPointerDown={(event) => event.stopPropagation()}
          size="compact"
        >
          <MoreHorizontal size={13} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" onClick={(event) => event.stopPropagation()}>
        <DropdownMenuItem onSelect={onToggleEnabled}>
          {enabled ? t("ssh.tasks.editor.disableStep") : t("ssh.tasks.editor.enableStep")}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={index === 0} onSelect={() => onMove(-1)}>
          {t("ssh.tasks.actions.moveUp")}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={index === stepCount - 1} onSelect={() => onMove(1)}>
          {t("ssh.tasks.actions.moveDown")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={onDuplicate}>
          {t("ssh.tasks.actions.duplicateStep")}
        </DropdownMenuItem>
        <DropdownMenuItem
          className="text-[var(--u-color-danger)]"
          onSelect={onRemove}
        >
          {t("ssh.tasks.actions.deleteStep")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function stepPreview(step: SshTaskStepInput) {
  const config = step.configJson as unknown as Record<string, unknown>;
  if (step.stepType === "command") {
    return String(config.command ?? "").trim();
  }
  const localPath = String(config.localPath ?? "").trim();
  const remotePath = String(config.remotePath ?? "").trim();
  if (!localPath && !remotePath) return "";
  if (step.stepType === "upload") {
    return `${localPath || "…"} → ${remotePath || "…"}`;
  }
  return `${remotePath || "…"} → ${localPath || "…"}`;
}

function Field({
  children,
  className = "",
  label,
}: {
  children: ReactNode;
  className?: string;
  label: string;
}) {
  return (
    <label className={`flex min-w-0 flex-col gap-1 ${className}`}>
      <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">{label}</span>
      {children}
    </label>
  );
}

function InlineField({ children, label }: { children: ReactNode; label: string }) {
  return (
    <label className="flex min-w-0 items-center gap-2">
      <span className="shrink-0 text-[11px] font-medium text-[var(--u-color-text-muted)]">
        {label}
      </span>
      {children}
    </label>
  );
}
