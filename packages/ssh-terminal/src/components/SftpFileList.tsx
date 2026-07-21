import { File, FileQuestion, Folder, Link2, RefreshCw } from "lucide-react";
import type { SftpFileEntry } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, LoadingState, cn, useI18n } from "@unfour/ui";
import { formatFileSize } from "../model/sftp-format";

export function SftpFileList({
  entries,
  error,
  loading,
  onActivate,
  onRetry,
  onSelect,
  selectedPath,
}: {
  entries: SftpFileEntry[];
  error: unknown;
  loading: boolean;
  onActivate: (entry: SftpFileEntry) => void;
  onRetry: () => void;
  onSelect: (entry: SftpFileEntry) => void;
  selectedPath: string | null;
}) {
  const { locale, t } = useI18n();

  if (loading && entries.length === 0) {
    return (
      <LoadingState className="min-h-0 flex-1 rounded-none border-0">
        {t("ssh.sftp.loadingDirectory")}
      </LoadingState>
    );
  }

  if (error && entries.length === 0) {
    return (
      <ErrorState className="min-h-0 flex-1 rounded-none border-0">
        <div className="flex flex-col items-center gap-2">
          <span>{sftpErrorMessage(error, t("ssh.sftp.loadFailed"))}</span>
          <Button onClick={onRetry} size="sm" type="button" variant="outline">
            <RefreshCw size={13} />
            {t("ssh.pane.retry")}
          </Button>
        </div>
      </ErrorState>
    );
  }

  if (entries.length === 0) {
    return (
      <EmptyState className="min-h-0 flex-1 rounded-none border-0">
        {t("ssh.sftp.emptyDirectory")}
      </EmptyState>
    );
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto" role="grid">
      <div className="sticky top-0 z-10 grid h-7 min-w-[440px] grid-cols-[minmax(150px,1fr)_72px_82px_124px] items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-medium text-[var(--u-color-text-muted)]">
        <span>{t("ssh.sftp.columns.name")}</span>
        <span>{t("ssh.sftp.columns.type")}</span>
        <span className="text-right">{t("ssh.sftp.columns.size")}</span>
        <span className="text-right">{t("ssh.sftp.columns.modified")}</span>
      </div>
      <div className="min-w-[440px]">
        {entries.map((entry) => {
          const selected = selectedPath === entry.path;
          return (
            <button
              aria-selected={selected}
              className={cn(
                "grid h-[30px] w-full grid-cols-[minmax(150px,1fr)_72px_82px_124px] items-center border-b border-[color:color-mix(in_srgb,var(--u-color-border)_55%,transparent)] px-2 text-left text-[12px] transition-colors duration-150",
                selected
                  ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-text)]"
                  : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
              )}
              key={entry.path}
              onClick={() => onSelect(entry)}
              onDoubleClick={() => onActivate(entry)}
              onKeyDown={(event) => {
                if (event.key === "Enter") onActivate(entry);
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
          );
        })}
      </div>
      {loading ? (
        <div className="sticky bottom-0 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 py-1 text-[11px] text-[var(--u-color-text-muted)]">
          {t("ssh.sftp.refreshing")}
        </div>
      ) : null}
    </div>
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
