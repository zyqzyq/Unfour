import { useState } from "react";
import type { FlowRun } from "@unfour/command-client";
import { Button, Popover, PopoverContent, PopoverTrigger, useI18n } from "@unfour/ui";

export function RunHistory({ runs, loading, failed, disabled, selected, onSelect, onRefresh }: {
  runs: FlowRun[]; loading: boolean; failed: boolean; disabled: boolean;
  selected: string | null; onSelect: (id: string) => void; onRefresh: () => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <Popover open={open} onOpenChange={(next) => { setOpen(next); if (next) onRefresh(); }}>
    <PopoverTrigger asChild><Button variant="secondary" disabled={disabled}>{t("flow.history")}</Button></PopoverTrigger>
    <PopoverContent align="end" aria-label={t("flow.history")} className="max-h-80 w-80 overflow-y-auto p-2">
      <strong>{t("flow.history")}</strong>
      {loading && <p className="py-2">{t("flow.loading")}</p>}
      {failed && <p role="alert">{t("flow.loadFailed")}</p>}
      {!loading && !failed && !runs.length && <p className="py-2">{t("flow.noRuns")}</p>}
      {[...runs].sort((a, b) => b.startedAt.localeCompare(a.startedAt)).map((run) => {
        const duration = run.finishedAt ? Date.parse(run.finishedAt) - Date.parse(run.startedAt) : null;
        return <Button key={run.id} variant="ghost" aria-pressed={selected === run.id} className="h-auto w-full justify-start py-2 text-left" onClick={() => { onSelect(run.id); setOpen(false); }}>
          <span className="grid gap-1"><span>{t(`flow.status.${run.status}`)} · <time dateTime={run.startedAt}>{run.startedAt}</time></span>
          {duration !== null && Number.isFinite(duration) && duration >= 0 && <span className="text-xs text-[var(--u-color-text-muted)]">{t("flow.elapsed")}: {duration} ms</span>}</span>
        </Button>;
      })}
    </PopoverContent>
  </Popover>;
}
