import { useEffect, useRef, useState } from "react";
import {
  previewSftpText,
  type SftpFileEntry,
  type SftpTextPreview,
  type SshSessionSummary,
} from "@unfour/command-client";
import {
  Button, Dialog, DialogBody, DialogClose, DialogContent, DialogDescription,
  DialogFooter, DialogHeader, DialogTitle, ErrorState, LoadingState, useI18n,
} from "@unfour/ui";
import { formatFileSize } from "../model/sftp-format";

type Props = {
  entry: SftpFileEntry;
  onClose: () => void;
  session: SshSessionSummary;
};

export function SftpPreviewDialog(props: Props) {
  const { entry, session } = props;
  // Replace all transient state synchronously when the request scope changes.
  return <PreviewContent {...props}
    key={`${session.workspaceId}:${session.sessionId}:${session.connectionId}:${session.status}:${entry.path}`} />;
}

function PreviewContent({ entry, onClose, session }: Props) {
  const { t } = useI18n();
  const [result, setResult] = useState<SftpTextPreview | null>(null);
  const [errorKey, setErrorKey] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<"copied" | "copyFailed" | null>(null);
  const request = useRef<Promise<SftpTextPreview> | null>(null);
  const connected = session.status === "connected";

  useEffect(() => {
    if (!connected) return;
    let active = true;
    // Component-local only: no React Query cache, Zustand store, or persistence.
    // Strict Mode can replay effects; share only this mounted preview's request.
    request.current ??= previewSftpText({
      workspaceId: session.workspaceId, sessionId: session.sessionId, path: entry.path,
    });
    void request.current.then((preview) => {
      if (active) setResult(preview);
    }).catch((error: unknown) => {
      if (active) setErrorKey(previewErrorKey(error));
    });
    return () => { active = false; };
  }, [connected, entry.path, session.sessionId, session.workspaceId]);

  const failure = connected ? errorKey : "disconnected";
  async function copy() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.content);
      setCopyStatus("copied");
    } catch {
      setCopyStatus("copyFailed");
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="w-[min(800px,calc(100vw-32px))]">
        <DialogHeader>
          <DialogTitle>{t("ssh.sftp.preview.title", { name: entry.name })}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <DialogDescription className="shrink-0 break-all">
            {entry.name} · {formatFileSize(result?.size ?? entry.size)} · {t("ssh.sftp.preview.description")}
          </DialogDescription>
          {failure ? (
            <ErrorState><span role="alert">{t(`ssh.sftp.preview.${failure}`)}</span></ErrorState>
          ) : result ? (
            <pre
              aria-label={t("ssh.sftp.preview.content")}
              className="min-h-0 overflow-auto whitespace-pre rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 font-mono text-[12px] select-text"
              tabIndex={0}
            >{result.content || t("ssh.sftp.preview.empty")}</pre>
          ) : (
            <LoadingState>{t("ssh.sftp.preview.loading")}</LoadingState>
          )}
        </DialogBody>
        <DialogFooter>
          {copyStatus ? <span role="status" className="mr-auto text-[12px] text-[var(--u-color-text-muted)]">
            {t(`ssh.sftp.preview.${copyStatus}`)}
          </span> : null}
          <Button disabled={!result || Boolean(failure)} onClick={() => void copy()} variant="outline">
            {t("ssh.sftp.preview.copy")}
          </Button>
          <DialogClose asChild><Button variant="ghost">{t("ssh.sftp.preview.close")}</Button></DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const errorKeys: Record<string, string> = {
  SFTP_PREVIEW_TOO_LARGE: "tooLarge",
  SFTP_PREVIEW_BINARY: "binary",
  SFTP_PREVIEW_ENCODING: "encoding",
  SFTP_PREVIEW_NOT_FILE: "notFile",
  SFTP_PREVIEW_PERMISSION_DENIED: "permissionDenied",
  SFTP_PREVIEW_DISCONNECTED: "disconnected",
  SFTP_PREVIEW_TIMEOUT: "timeout",
  SFTP_PREVIEW_NOT_FOUND: "notFound",
};

function previewErrorKey(error: unknown) {
  const code = error && typeof error === "object" && "code" in error ? error.code : null;
  return typeof code === "string" ? errorKeys[code] ?? "failed" : "failed";
}
