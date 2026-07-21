import {
  ArrowUp,
  Download,
  FolderOpen,
  FolderPlus,
  PanelRightClose,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
} from "lucide-react";
import type { SftpFileEntry } from "@unfour/command-client";
import { IconButton, Input, useI18n } from "@unfour/ui";

export function SftpToolbar({
  canRefresh,
  connected,
  currentPath,
  endpoint,
  onClose,
  onDelete,
  onDownload,
  onNewFolder,
  onParent,
  onPathChange,
  onPathSubmit,
  onRefresh,
  onRename,
  onUpload,
  opening,
  pathValue,
  refreshing,
  selectedEntry,
}: {
  canRefresh: boolean;
  connected: boolean;
  currentPath: string | null;
  endpoint: string;
  onClose: () => void;
  onDelete: () => void;
  onDownload: () => void;
  onNewFolder: () => void;
  onParent: () => void;
  onPathChange: (value: string) => void;
  onPathSubmit: () => void;
  onRefresh: () => void;
  onRename: () => void;
  onUpload: () => void;
  opening: boolean;
  pathValue: string;
  refreshing: boolean;
  selectedEntry: SftpFileEntry | null;
}) {
  const { t } = useI18n();
  return (
    <>
      <div className="flex h-[34px] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-1.5">
        <IconButton label={t("ssh.sftp.closePanel")} onClick={onClose} size="compact">
          <PanelRightClose size={14} />
        </IconButton>
        <FolderOpen className="shrink-0 text-[var(--u-color-primary)]" size={14} />
        <div className="min-w-0 flex-1 truncate text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("ssh.sftp.panelTitle")}
        </div>
        <div
          className="max-w-[44%] truncate text-[11px] text-[var(--u-color-text-muted)]"
          title={endpoint}
        >
          {endpoint}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-0.5 border-b border-[var(--u-color-border)] px-1.5 py-1">
        <IconButton
          disabled={!connected || !currentPath || currentPath === "/"}
          label={t("ssh.sftp.parentDirectory")}
          onClick={onParent}
          size="compact"
        >
          <ArrowUp size={14} />
        </IconButton>
        <IconButton
          disabled={!connected || !canRefresh}
          label={t("ssh.sftp.refresh")}
          onClick={onRefresh}
          size="compact"
        >
          <RefreshCw className={refreshing ? "animate-spin" : undefined} size={14} />
        </IconButton>
        <span className="mx-0.5 h-4 w-px bg-[var(--u-color-border)]" />
        <IconButton
          disabled={!connected || !currentPath}
          label={t("ssh.sftp.upload")}
          onClick={onUpload}
          size="compact"
        >
          <Upload size={14} />
        </IconButton>
        <IconButton
          disabled={!connected || selectedEntry?.kind !== "file"}
          label={t("ssh.sftp.download")}
          onClick={onDownload}
          size="compact"
        >
          <Download size={14} />
        </IconButton>
        <IconButton
          disabled={!connected || !currentPath}
          label={t("ssh.sftp.newFolder")}
          onClick={onNewFolder}
          size="compact"
        >
          <FolderPlus size={14} />
        </IconButton>
        <IconButton
          disabled={!connected || !selectedEntry}
          label={t("ssh.sftp.rename")}
          onClick={onRename}
          size="compact"
        >
          <Pencil size={14} />
        </IconButton>
        <IconButton
          className="hover:text-[var(--u-color-danger)]"
          disabled={!connected || !selectedEntry}
          label={t("ssh.sftp.delete")}
          onClick={onDelete}
          size="compact"
        >
          <Trash2 size={14} />
        </IconButton>
      </div>
      <form
        className="shrink-0 border-b border-[var(--u-color-border)] p-1.5"
        onSubmit={(event) => {
          event.preventDefault();
          if (pathValue.trim()) onPathSubmit();
        }}
      >
        <Input
          aria-label={t("ssh.sftp.path")}
          className="h-7 font-mono text-[12px]"
          disabled={!connected || opening}
          onChange={(event) => onPathChange(event.target.value)}
          placeholder="/"
          value={pathValue}
        />
      </form>
    </>
  );
}
