import {
  ChevronDown,
  Columns2,
  Copy,
  CircleX,
  Download,
  Eraser,
  FilePlus2,
  MoreHorizontal,
  Rows2,
  Scaling,
  Search,
  SquareSplitHorizontal,
  TerminalSquare,
  Unplug,
} from "lucide-react";
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  StatusBadge,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import type { TerminalSplitMode } from "../model/types";

export function TerminalModuleToolbar({
  activeSessionCount,
  canConnect,
  canSplit,
  canUseSessionActions,
  connecting,
  reconnecting,
  onCancelReconnect,
  onClear,
  onCloseSession,
  onCopyLog,
  onExportLog,
  onNewConnection,
  onNewSession,
  onOpenPreferences,
  onResize,
  onSearch,
  onSplit,
  selectedConnectionName,
  splitMode,
}: {
  activeSessionCount: number;
  canConnect: boolean;
  canSplit: boolean;
  canUseSessionActions: boolean;
  connecting?: boolean;
  reconnecting?: boolean;
  onCancelReconnect: () => void;
  onClear: () => void;
  onCloseSession: () => void;
  onCopyLog: () => void;
  onExportLog: () => void;
  onNewConnection: () => void;
  onNewSession: () => void;
  onOpenPreferences: () => void;
  onResize?: () => void;
  onSearch: () => void;
  onSplit: (mode: TerminalSplitMode) => void;
  selectedConnectionName?: string;
  splitMode: TerminalSplitMode;
}) {
  const { t } = useI18n();

  return (
    <Toolbar className="overflow-x-auto">
      <ToolbarGroup className="gap-2">
        <TerminalSquare size={15} />
        <span className="max-w-[220px] truncate text-[12px] font-semibold text-[var(--u-color-text)] max-[900px]:hidden">
          {selectedConnectionName ?? t("ssh.status.terminalSsh")}
        </span>
        <StatusBadge className="max-[900px]:hidden">
          {t("ssh.status.sessionCount", { count: activeSessionCount })}
        </StatusBadge>
      </ToolbarGroup>
      <ToolbarGroup>
        <Button
          aria-label={t("ssh.actions.newConnectionAria")}
          onClick={onNewConnection}
          size="sm"
          type="button"
          variant="outline"
        >
          <FilePlus2 size={14} />
          <span className="max-[900px]:hidden">{t("ssh.actions.newConnection")}</span>
        </Button>
        <Button
          aria-label={t("ssh.actions.newSessionAria")}
          disabled={!canConnect || connecting}
          onClick={onNewSession}
          size="sm"
          type="button"
        >
          <TerminalSquare size={14} />
          <span className="max-[900px]:hidden">
            {connecting ? t("common.actions.connecting") : t("ssh.actions.newSession")}
          </span>
        </Button>
        {reconnecting && (
          <Button onClick={onCancelReconnect} size="sm" type="button" variant="outline">
            <CircleX size={14} />
            <span className="max-[900px]:hidden">{t("ssh.actions.cancelReconnect")}</span>
          </Button>
        )}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={t("ssh.actions.splitPane")}
              disabled={!canSplit}
              size="sm"
              type="button"
              variant="outline"
            >
              {splitMode === "horizontal" ? <Rows2 size={14} /> : <Columns2 size={14} />}
              <span className="max-[900px]:hidden">{t("ssh.actions.splitPane")}</span>
              <ChevronDown className="max-[900px]:hidden" size={13} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => onSplit("single")}>
              <SquareSplitHorizontal size={13} />
              {t("ssh.actions.singlePane")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onSplit("vertical")}>
              <Columns2 size={13} />
              {t("ssh.actions.splitRight")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onSplit("horizontal")}>
              <Rows2 size={13} />
              {t("ssh.actions.splitDown")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <IconButton label={t("ssh.actions.searchOutput")} onClick={onSearch}>
          <Search size={14} />
        </IconButton>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label={t("ssh.actions.terminalActions")}>
              <MoreHorizontal size={15} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onCloseSession}>
              <Unplug size={13} />
              {t("ssh.actions.closeSession")}
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onClear}>
              <Eraser size={13} />
              {t("ssh.actions.clearTerminal")}
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onCopyLog}>
              <Copy size={13} />
              {t("ssh.actions.copySessionLog")}
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions || !onResize} onSelect={onResize}>
              <Scaling size={13} />
              {t("ssh.actions.resizePty")}
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onExportLog}>
              <Download size={13} />
              {t("ssh.actions.exportLogs")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={onOpenPreferences}>
              {t("ssh.actions.preferences")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </ToolbarGroup>
    </Toolbar>
  );
}
