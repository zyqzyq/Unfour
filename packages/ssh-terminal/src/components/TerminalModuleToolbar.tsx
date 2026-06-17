import {
  ChevronDown,
  Columns2,
  Copy,
  CircleX,
  Download,
  FilePlus2,
  MoreHorizontal,
  RotateCw,
  Rows2,
  Search,
  SquareSplitHorizontal,
  TerminalSquare,
  Trash2,
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
  return (
    <Toolbar className="overflow-x-auto">
      <ToolbarGroup className="gap-2">
        <TerminalSquare size={15} />
        <span className="max-w-[220px] truncate text-[12px] font-semibold text-[var(--u-color-text)] max-[900px]:hidden">
          {selectedConnectionName ?? "Terminal / SSH"}
        </span>
        <StatusBadge className="max-[900px]:hidden">{activeSessionCount} sessions</StatusBadge>
      </ToolbarGroup>
      <ToolbarGroup>
        <Button
          aria-label="New SSH connection"
          onClick={onNewConnection}
          size="sm"
          type="button"
          variant="outline"
        >
          <FilePlus2 size={14} />
          <span className="max-[900px]:hidden">New Connection</span>
        </Button>
        <Button
          aria-label="New terminal session"
          disabled={!canConnect || connecting}
          onClick={onNewSession}
          size="sm"
          type="button"
        >
          <TerminalSquare size={14} />
          <span className="max-[900px]:hidden">
            {connecting ? "Connecting" : "New Session"}
          </span>
        </Button>
        <IconButton
          disabled={!canConnect || connecting}
          label="Reconnect SSH session"
          onClick={onNewSession}
        >
          <RotateCw size={14} />
        </IconButton>
        {reconnecting && (
          <Button onClick={onCancelReconnect} size="sm" type="button" variant="outline">
            <CircleX size={14} />
            <span className="max-[900px]:hidden">Cancel Reconnect</span>
          </Button>
        )}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label="Split terminal pane"
              disabled={!canSplit}
              size="sm"
              type="button"
              variant="outline"
            >
              {splitMode === "horizontal" ? <Rows2 size={14} /> : <Columns2 size={14} />}
              <span className="max-[900px]:hidden">Split Pane</span>
              <ChevronDown className="max-[900px]:hidden" size={13} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => onSplit("single")}>
              <SquareSplitHorizontal size={13} />
              Single Pane
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onSplit("vertical")}>
              <Columns2 size={13} />
              Split Right
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onSplit("horizontal")}>
              <Rows2 size={13} />
              Split Down
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <IconButton label="Search terminal output" onClick={onSearch}>
          <Search size={14} />
        </IconButton>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label="Terminal actions">
              <MoreHorizontal size={15} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onCloseSession}>
              <Trash2 size={13} />
              Close Session
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onClear}>
              <CircleX size={13} />
              Clear Terminal
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onCopyLog}>
              <Copy size={13} />
              Copy Session Log
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions || !onResize} onSelect={onResize}>
              <RotateCw size={13} />
              Resize PTY
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!canUseSessionActions} onSelect={onExportLog}>
              <Download size={13} />
              Export Logs
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={onOpenPreferences}>Terminal Preferences</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </ToolbarGroup>
    </Toolbar>
  );
}
