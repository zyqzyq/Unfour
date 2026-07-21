import type { SshTaskRun } from "@unfour/command-client";
import { Button, LoadingState, StatusBadge, useI18n } from "@unfour/ui";
import { Trash2 } from "lucide-react";

export function TaskHistory({
  clearing,
  loading,
  onClear,
  runs,
}: {
  clearing: boolean;
  loading: boolean;
  onClear: () => void;
  runs: SshTaskRun[];
}) {
  const { t } = useI18n();
  if (loading) {
    return <LoadingState>{t("ssh.tasks.history.loading")}</LoadingState>;
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <span className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("ssh.tasks.history.title")}
        </span>
        <Button disabled={!runs.length || clearing} onClick={onClear} size="sm" variant="danger">
          <Trash2 size={12} />
          {clearing ? t("ssh.tasks.history.clearing") : t("ssh.tasks.history.clear")}
        </Button>
      </div>
      {runs.length ? (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full table-fixed text-left text-[12px]">
            <thead className="sticky top-0 bg-[var(--u-color-surface-subtle)] text-[11px] text-[var(--u-color-text-muted)]">
              <tr className="h-8 border-b border-[var(--u-color-border)]">
                <th className="w-28 px-2 font-medium">{t("ssh.tasks.history.status")}</th>
                <th className="w-44 px-2 font-medium">{t("ssh.tasks.history.started")}</th>
                <th className="w-44 px-2 font-medium">{t("ssh.tasks.history.finished")}</th>
                <th className="px-2 font-medium">{t("ssh.tasks.history.error")}</th>
                <th className="w-[34%] px-2 font-medium">{t("ssh.tasks.history.logPath")}</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => (
                <tr className="h-9 border-b border-[var(--u-color-border)] text-[var(--u-color-text)]" key={run.id}>
                  <td className="px-2"><StatusBadge tone={tone(run.status)}>{t(`ssh.tasks.run.status.${run.status}`)}</StatusBadge></td>
                  <td className="truncate px-2">{formatDate(run.startedAt)}</td>
                  <td className="truncate px-2">{run.finishedAt ? formatDate(run.finishedAt) : "—"}</td>
                  <td className="truncate px-2 text-[var(--u-color-danger)]" title={run.errorMessage ?? undefined}>{run.errorMessage ?? "—"}</td>
                  <td className="truncate px-2 font-mono text-[11px] text-[var(--u-color-text-muted)]" title={run.logPath}>{run.logPath}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center text-[12px] text-[var(--u-color-text-muted)]">
          {t("ssh.tasks.history.empty")}
        </div>
      )}
    </div>
  );
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(new Date(value));
}

function tone(status: SshTaskRun["status"]): "neutral" | "success" | "warning" | "danger" {
  if (status === "success") return "success";
  if (status === "failed") return "danger";
  if (status === "cancelled") return "warning";
  return "neutral";
}
