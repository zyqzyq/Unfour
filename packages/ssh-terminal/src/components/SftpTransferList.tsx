import { Ban, CheckCircle2, Download, Loader2, Upload, XCircle } from "lucide-react";
import type { SftpTransferState } from "@unfour/command-client";
import { IconButton, cn, useI18n } from "@unfour/ui";
import { formatFileSize } from "../model/sftp-format";

export function SftpTransferList({
  onCancel,
  onReveal,
  transfers,
}: {
  onCancel: (transfer: SftpTransferState) => void;
  onReveal: (transfer: SftpTransferState) => void;
  transfers: SftpTransferState[];
}) {
  const { t } = useI18n();
  if (transfers.length === 0) return null;

  return (
    <section className="max-h-40 shrink-0 overflow-auto border-t border-[var(--u-color-border)]">
      <div className="sticky top-0 z-10 flex h-7 items-center bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold text-[var(--u-color-text-muted)]">
        {t("ssh.sftp.transfers")}
      </div>
      {transfers.slice(0, 8).map((transfer) => {
        const running = transfer.status === "pending" || transfer.status === "running";
        const progress = transfer.totalBytes
          ? Math.min(100, (transfer.transferredBytes / transfer.totalBytes) * 100)
          : 0;
        return (
          <div
            className="border-t border-[var(--u-color-border)] px-2 py-1.5 text-[11px]"
            key={transfer.transferId}
          >
            <div className="flex items-center gap-1.5">
              <TransferIcon transfer={transfer} />
              <button
                className={cn(
                  "min-w-0 flex-1 truncate text-left font-medium text-[var(--u-color-text)]",
                  transfer.direction === "download" && transfer.status === "success"
                    ? "cursor-pointer hover:underline"
                    : "cursor-default",
                )}
                disabled={transfer.direction !== "download" || transfer.status !== "success"}
                onClick={() => onReveal(transfer)}
                title={transfer.direction === "download" ? transfer.localPath : transfer.remotePath}
                type="button"
              >
                {fileName(
                  transfer.direction === "download" ? transfer.remotePath : transfer.localPath,
                )}
              </button>
              {running ? (
                <IconButton
                  className="h-6 w-6"
                  label={t("ssh.sftp.cancelTransfer")}
                  onClick={() => onCancel(transfer)}
                  size="compact"
                >
                  <Ban size={12} />
                </IconButton>
              ) : null}
            </div>
            <div className="mt-1 flex items-center justify-between gap-2 text-[10px] text-[var(--u-color-text-muted)]">
              <span>
                {formatFileSize(transfer.transferredBytes)} / {formatFileSize(transfer.totalBytes)}
              </span>
              <span>
                {running ? `${Math.round(progress)}% · ${formatFileSize(transfer.bytesPerSecond)}/s` : t(`ssh.sftp.transferStatus.${transfer.status}`)}
              </span>
            </div>
            {running ? (
              <div className="mt-1 h-1 overflow-hidden rounded-full bg-[var(--u-color-surface-muted)]">
                <div
                  className="h-full bg-[var(--u-color-primary)] transition-[width] duration-150"
                  style={{ width: `${progress}%` }}
                />
              </div>
            ) : null}
            {transfer.error ? (
              <div className="mt-1 truncate text-[10px] text-[var(--u-color-danger)]" title={transfer.error}>
                {transfer.error}
              </div>
            ) : null}
          </div>
        );
      })}
    </section>
  );
}

function TransferIcon({ transfer }: { transfer: SftpTransferState }) {
  if (transfer.status === "running" || transfer.status === "pending") {
    return <Loader2 className="shrink-0 animate-spin text-[var(--u-color-primary)]" size={13} />;
  }
  if (transfer.status === "success") {
    return <CheckCircle2 className="shrink-0 text-[var(--u-color-success)]" size={13} />;
  }
  if (transfer.status === "failed") {
    return <XCircle className="shrink-0 text-[var(--u-color-danger)]" size={13} />;
  }
  return transfer.direction === "upload" ? <Upload size={13} /> : <Download size={13} />;
}

function fileName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
