import type { TemporaryTaskExecutionMode } from "@unfour/command-client";
import { Select, useI18n } from "@unfour/ui";

export function TemporaryTaskExecutionControls({ mode, ready, busy, recoveryRequired, commandOnly, pending, onModeChange, onReadyChange }: {
  mode: TemporaryTaskExecutionMode;
  ready: boolean;
  busy: boolean;
  recoveryRequired: boolean;
  commandOnly: boolean;
  pending: boolean;
  onModeChange?: (mode: TemporaryTaskExecutionMode) => void;
  onReadyChange?: (ready: boolean) => void;
}) {
  const { t } = useI18n();
  return <section className="flex flex-col gap-2 text-[12px]">
    <label className="flex flex-col gap-1">
      <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">{t("ssh.tasks.run.executionMode")}</span>
      <Select value={mode} disabled={pending} onChange={(event) => onModeChange?.(event.target.value === "terminal" ? "terminal" : "exec")}>
        <option value="terminal">{t("ssh.tasks.run.modeTerminal")}</option>
        <option value="exec">{t("ssh.tasks.run.modeExec")}</option>
      </Select>
    </label>
    <p className="text-[var(--u-color-text-muted)]">{t(mode === "terminal" ? "ssh.tasks.run.terminalHelp" : "ssh.tasks.run.execHelp")}</p>
    {mode === "terminal" && <label className="flex items-start gap-2">
      <input type="checkbox" className="mt-0.5" checked={ready} disabled={pending || busy || recoveryRequired || !commandOnly} onChange={(event) => onReadyChange?.(event.target.checked)} />
      <span>{t("ssh.tasks.run.terminalReady")}</span>
    </label>}
    {mode === "terminal" && !commandOnly && <p role="alert" className="text-[var(--u-color-warning)]">{t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_COMMAND_ONLY")}</p>}
    {busy && <p role="status" className="text-[var(--u-color-warning)]">{t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_BUSY")}</p>}
    {mode === "terminal" && recoveryRequired && <p role="alert" className="text-[var(--u-color-warning)]">{t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_RECOVERY_REQUIRED")}</p>}
  </section>;
}
