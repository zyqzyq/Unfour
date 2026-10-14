import { Activity, Check, Copy, Download, LoaderCircle, Minus } from "lucide-react";
import { useEffect, useMemo } from "react";
import { useMutation } from "@tanstack/react-query";
import { exportSshLog } from "@unfour/command-client";
import {
  BottomPanel,
  EmptyState,
  IconButton,
  StatusBadge,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import { useTerminalStore, redactTerminalLog } from "../model/terminal-state";

const EMPTY_EVENTS: never[] = [];

export function TerminalLogPanel({
  collapsed,
  height,
  onCollapse,
  onHeightChange,
  workspaceId,
}: {
  collapsed?: boolean;
  height?: number;
  onCollapse: () => void;
  onHeightChange: (height: number) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const activeSessionId = useTerminalStore((state) => state.activeSessionId);
  const eventCount = useTerminalStore((state) => state.terminalEvents.length);
  // Collapsed by default in the shell. Skip rebuilding the full log string on
  // every PTY chunk while the panel is hidden (critical when Tasks keeps SSH
  // mounted with live sessions underneath).
  const events = useTerminalStore((state) =>
    collapsed ? EMPTY_EVENTS : state.terminalEvents,
  );
  const exportedLog = useTerminalStore((state) => state.exportedLog);
  const setExportedLog = useTerminalStore((state) => state.setExportedLog);
  const visibleLog = useMemo(() => {
    if (exportedLog) {
      return redactTerminalLog(exportedLog);
    }
    if (collapsed) {
      return "";
    }
    const logLines = events.map(
      (event) =>
        `[${event.createdAt}] ${event.kind} ${redactTerminalLog(event.data).trim()}`,
    );
    return redactTerminalLog(logLines.join("\n"));
  }, [collapsed, events, exportedLog]);

  const exportMutation = useMutation({
    mutationFn: () => exportSshLog({ workspaceId, sessionId: activeSessionId ?? "" }),
    onSuccess: (log) => setExportedLog(log.content),
  });
  const copyMutation = useMutation({
    mutationFn: async (content: string) => {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard unavailable");
      await navigator.clipboard.writeText(content);
    },
  });
  const { isSuccess: copied, reset: resetCopy } = copyMutation;
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(resetCopy, 1800);
    return () => window.clearTimeout(timer);
  }, [copied, resetCopy]);

  return (
    <BottomPanel
      className="flex flex-col"
      collapsed={collapsed}
      height={height}
      onHeightChange={onHeightChange}
      resizable
    >
      <Toolbar>
        <ToolbarGroup>
          <Activity size={14} />
          <span className="text-[12px] font-semibold text-[var(--u-color-text)]">
            {t("ssh.logs.title")}
          </span>
          <StatusBadge>{eventCount}</StatusBadge>
        </ToolbarGroup>
        <ToolbarGroup>
          <IconButton
            disabled={!visibleLog || copyMutation.isPending}
            label={t("ssh.logs.copy")}
            onClick={() => copyMutation.mutate(visibleLog)}
          >
            {copyMutation.isPending ? <LoaderCircle className="animate-spin" size={14} /> : copied ? <Check size={14} /> : <Copy size={14} />}
          </IconButton>
          <IconButton
            disabled={!activeSessionId || exportMutation.isPending}
            label={t(exportMutation.isPending ? "ssh.logs.exporting" : "ssh.logs.export")}
            onClick={() => exportMutation.mutate()}
          >
            {exportMutation.isPending ? <LoaderCircle className="animate-spin" size={14} /> : <Download size={14} />}
          </IconButton>
          <IconButton label={t("ssh.logs.collapse")} onClick={onCollapse}>
            <Minus size={14} />
          </IconButton>
        </ToolbarGroup>
      </Toolbar>
      {copyMutation.isError ? (
        <p role="alert" className="px-2 py-1 text-xs text-[var(--u-color-danger)]">{t("ssh.logs.copyFailed")}</p>
      ) : copied && (
        <p role="status" className="px-2 py-1 text-xs text-[var(--u-color-success)]">{t("ssh.logs.copied")}</p>
      )}
      {exportMutation.isError && <p role="alert" className="px-2 py-1 text-xs text-[var(--u-color-danger)]">{t("ssh.logs.exportFailed")}</p>}
      <div className="min-h-0 flex-1 overflow-auto p-2">
        {visibleLog ? (
          <pre className="min-h-full whitespace-pre-wrap break-words font-mono text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {visibleLog}
          </pre>
        ) : (
          <EmptyState className="h-full min-h-0">
            <div className="space-y-1">
              <p>{t("ssh.logs.empty")}</p>
              <p>{t("ssh.logs.emptyHelp")}</p>
            </div>
          </EmptyState>
        )}
      </div>
    </BottomPanel>
  );
}
