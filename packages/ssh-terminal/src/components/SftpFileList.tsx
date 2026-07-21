import type { ReactNode } from "react";
import { File, FileQuestion, Folder, Link2, RefreshCw } from "lucide-react";
import type { SftpFileEntry } from "@unfour/command-client";
import {
  Button,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  EmptyState,
  ErrorState,
  LoadingState,
  cn,
  useI18n,
} from "@unfour/ui";
import { formatFileSize } from "../model/sftp-format";

export type SftpFileListActions = {
  canGoParent: boolean;
  canRefresh: boolean;
  canUpload: boolean;
  onCopyPath: (entry: SftpFileEntry) => void;
  onDelete: (entry: SftpFileEntry) => void;
  onDownload: (entry: SftpFileEntry) => void;
  onNewFolder: () => void;
  onOpen: (entry: SftpFileEntry) => void;
  onParent: () => void;
  onRefresh: () => void;
  onRename: (entry: SftpFileEntry) => void;
  onUpload: () => void;
  onUploadHere: (entry: SftpFileEntry) => void;
};

export function SftpFileList({
  actions,
  entries,
  error,
  loading,
  onActivate,
  onRetry,
  onSelect,
  onSelectRange,
  onToggleSelect,
  selectedPaths,
}: {
  actions: SftpFileListActions;
  entries: SftpFileEntry[];
  error: unknown;
  loading: boolean;
  onActivate: (entry: SftpFileEntry) => void;
  onRetry: () => void;
  onSelect: (entry: SftpFileEntry) => void;
  onSelectRange: (entry: SftpFileEntry) => void;
  onToggleSelect: (entry: SftpFileEntry) => void;
  selectedPaths: string[];
}) {
  const { locale, t } = useI18n();
  const selectedSet = new Set(selectedPaths);

  if (loading && entries.length === 0) {
    return (
      <BlankContextMenu actions={actions}>
        <LoadingState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.sftp.loadingDirectory")}
        </LoadingState>
      </BlankContextMenu>
    );
  }

  if (error && entries.length === 0) {
    return (
      <BlankContextMenu actions={actions}>
        <ErrorState className="min-h-0 flex-1 rounded-none border-0">
          <div className="flex flex-col items-center gap-2">
            <span>{sftpErrorMessage(error, t("ssh.sftp.loadFailed"))}</span>
            <Button onClick={onRetry} size="sm" type="button" variant="outline">
              <RefreshCw size={13} />
              {t("ssh.pane.retry")}
            </Button>
          </div>
        </ErrorState>
      </BlankContextMenu>
    );
  }

  if (entries.length === 0) {
    return (
      <BlankContextMenu actions={actions}>
        <EmptyState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.sftp.emptyDirectory")}
        </EmptyState>
      </BlankContextMenu>
    );
  }

  return (
    <BlankContextMenu actions={actions}>
      <div className="min-h-0 flex-1 overflow-auto" role="grid">
        <div className="sticky top-0 z-10 grid h-7 min-w-[440px] grid-cols-[minmax(150px,1fr)_72px_82px_124px] items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-medium text-[var(--u-color-text-muted)]">
          <span>{t("ssh.sftp.columns.name")}</span>
          <span>{t("ssh.sftp.columns.type")}</span>
          <span className="text-right">{t("ssh.sftp.columns.size")}</span>
          <span className="text-right">{t("ssh.sftp.columns.modified")}</span>
        </div>
        <div className="min-w-[440px]">
          {entries.map((entry) => {
            const selected = selectedSet.has(entry.path);
            return (
              <EntryContextMenu actions={actions} entry={entry} key={entry.path} onSelect={onSelect}>
                <button
                  aria-selected={selected}
                  className={cn(
                    "grid h-[30px] w-full grid-cols-[minmax(150px,1fr)_72px_82px_124px] items-center border-b border-[color:color-mix(in_srgb,var(--u-color-border)_55%,transparent)] px-2 text-left text-[12px] transition-colors duration-150",
                    selected
                      ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-text)]"
                      : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
                  )}
                  data-sftp-path={entry.path}
                  onClick={(event) => {
                    if (event.shiftKey) {
                      onSelectRange(entry);
                      return;
                    }
                    if (event.metaKey || event.ctrlKey) {
                      onToggleSelect(entry);
                      return;
                    }
                    onSelect(entry);
                  }}
                  onContextMenu={() => onSelect(entry)}
                  onDoubleClick={() => onActivate(entry)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      onActivate(entry);
                      return;
                    }
                    if ((event.key === "F10" && event.shiftKey) || event.key === "ContextMenu") {
                      event.preventDefault();
                      const rect = event.currentTarget.getBoundingClientRect();
                      event.currentTarget.dispatchEvent(
                        new MouseEvent("contextmenu", {
                          bubbles: true,
                          cancelable: true,
                          clientX: Math.round(rect.left + Math.min(24, rect.width / 2)),
                          clientY: Math.round(rect.top + rect.height / 2),
                        }),
                      );
                    }
                  }}
                  role="row"
                  title={entry.name}
                  type="button"
                >
                  <span className="flex min-w-0 items-center gap-1.5">
                    <FileKindIcon entry={entry} />
                    <span className="truncate font-medium">{entry.name}</span>
                  </span>
                  <span className="truncate">{t(`ssh.sftp.types.${entry.kind}`)}</span>
                  <span className="truncate text-right font-mono">
                    {entry.kind === "directory" ? "—" : formatFileSize(entry.size)}
                  </span>
                  <span className="truncate text-right" title={entry.modifiedAt ?? undefined}>
                    {formatModifiedTime(entry.modifiedAt, locale)}
                  </span>
                </button>
              </EntryContextMenu>
            );
          })}
        </div>
        {loading ? (
          <div className="sticky bottom-0 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 py-1 text-[11px] text-[var(--u-color-text-muted)]">
            {t("ssh.sftp.refreshing")}
          </div>
        ) : null}
      </div>
    </BlankContextMenu>
  );
}

function BlankContextMenu({
  actions,
  children,
}: {
  actions: SftpFileListActions;
  children: ReactNode;
}) {
  const { t } = useI18n();
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div className="flex min-h-0 flex-1 flex-col">{children}</div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem disabled={!actions.canUpload} onSelect={actions.onUpload}>
          {t("ssh.sftp.menu.upload")}
        </ContextMenuItem>
        <ContextMenuItem disabled={!actions.canUpload} onSelect={actions.onNewFolder}>
          {t("ssh.sftp.menu.newFolder")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem disabled={!actions.canRefresh} onSelect={actions.onRefresh}>
          {t("ssh.sftp.menu.refresh")}
        </ContextMenuItem>
        <ContextMenuItem disabled={!actions.canGoParent} onSelect={actions.onParent}>
          {t("ssh.sftp.menu.parentDirectory")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function EntryContextMenu({
  actions,
  children,
  entry,
  onSelect,
}: {
  actions: SftpFileListActions;
  children: ReactNode;
  entry: SftpFileEntry;
  onSelect: (entry: SftpFileEntry) => void;
}) {
  const { t } = useI18n();
  const isDirectory = entry.kind === "directory";
  const isFile = entry.kind === "file";

  return (
    <ContextMenu onOpenChange={(open) => open && onSelect(entry)}>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        {isDirectory ? (
          <>
            <ContextMenuItem onSelect={() => actions.onOpen(entry)}>
              {t("ssh.sftp.menu.open")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem
              disabled={!actions.canUpload}
              onSelect={() => actions.onUploadHere(entry)}
            >
              {t("ssh.sftp.menu.uploadHere")}
            </ContextMenuItem>
            <ContextMenuItem disabled={!actions.canUpload} onSelect={actions.onNewFolder}>
              {t("ssh.sftp.menu.newFolder")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => actions.onRename(entry)}>
              {t("ssh.sftp.menu.rename")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => actions.onDelete(entry)} tone="danger">
              {t("ssh.sftp.menu.delete")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => actions.onCopyPath(entry)}>
              {t("ssh.sftp.menu.copyPath")}
            </ContextMenuItem>
          </>
        ) : (
          <>
            {isFile ? (
              <>
                <ContextMenuItem onSelect={() => actions.onDownload(entry)}>
                  {t("ssh.sftp.menu.download")}
                </ContextMenuItem>
                <ContextMenuSeparator />
              </>
            ) : null}
            <ContextMenuItem onSelect={() => actions.onRename(entry)}>
              {t("ssh.sftp.menu.rename")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => actions.onDelete(entry)} tone="danger">
              {t("ssh.sftp.menu.delete")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => actions.onCopyPath(entry)}>
              {t("ssh.sftp.menu.copyPath")}
            </ContextMenuItem>
          </>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}

function FileKindIcon({ entry }: { entry: SftpFileEntry }) {
  if (entry.kind === "directory") return <Folder className="shrink-0" size={14} />;
  if (entry.kind === "symlink") return <Link2 className="shrink-0" size={14} />;
  if (entry.kind === "file") return <File className="shrink-0" size={14} />;
  return <FileQuestion className="shrink-0" size={14} />;
}

function formatModifiedTime(value: string | null, locale: string) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function sftpErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error && error.message ? error.message : fallback;
}
