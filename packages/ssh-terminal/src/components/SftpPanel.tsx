import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  cancelSftpTransfer,
  createSftpDirectory,
  deleteSftpPath,
  downloadSftpFile,
  listSftpDirectory,
  openSftp,
  renameSftpPath,
  uploadSftpFile,
  type SftpFileEntry,
  type SftpTransferState,
  type SshSessionSummary,
} from "@unfour/command-client";
import {
  ConfirmDialog,
  ErrorState,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import { useSftpStore } from "../model/sftp-state";
import { SftpFileList } from "./SftpFileList";
import { SftpNameDialog } from "./SftpNameDialog";
import { SftpTransferList } from "./SftpTransferList";
import { SftpToolbar } from "./SftpToolbar";

type NameAction = "mkdir" | "rename" | "upload";
const EMPTY_TRANSFERS: SftpTransferState[] = [];

export function SftpPanel({
  onClose,
  session,
}: {
  onClose: () => void;
  session: SshSessionSummary;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const tab = useSftpStore((state) => state.tabs[session.sessionId]);
  const transfers = useSftpStore(
    (state) => state.transfers[session.sessionId] ?? EMPTY_TRANSFERS,
  );
  const setPanelPath = useSftpStore((state) => state.setPanelPath);
  const setSelectedPath = useSftpStore((state) => state.setSelectedPath);
  const upsertTransfer = useSftpStore((state) => state.upsertTransfer);
  const [pathDraft, setPathDraft] = useState<{
    dirty: boolean;
    sessionId: string;
    value: string;
  }>({ dirty: false, sessionId: session.sessionId, value: "" });
  const [nameAction, setNameAction] = useState<NameAction | null>(null);
  const [nameValue, setNameValue] = useState("");
  const [localUploadPath, setLocalUploadPath] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<SftpFileEntry | null>(null);
  const connected = session.status === "connected";

  const openQuery = useQuery({
    enabled: connected,
    queryKey: ["ssh-sftp-open", session.workspaceId, session.sessionId],
    queryFn: () =>
      openSftp({ workspaceId: session.workspaceId, sessionId: session.sessionId }),
    refetchOnMount: "always",
    retry: false,
    staleTime: 0,
  });
  const currentPath =
    tab?.connectionId === session.connectionId && tab.path
      ? tab.path
      : openQuery.data?.homePath ?? null;
  const selectedPath =
    tab?.connectionId === session.connectionId ? tab.selectedPath : null;
  const pathValue =
    pathDraft.sessionId === session.sessionId && pathDraft.dirty
      ? pathDraft.value
      : currentPath ?? "";

  const directoryQuery = useQuery({
    enabled: connected && openQuery.isSuccess && Boolean(currentPath),
    queryKey: ["ssh-sftp-directory", session.workspaceId, session.sessionId, currentPath],
    queryFn: () =>
      listSftpDirectory({
        workspaceId: session.workspaceId,
        sessionId: session.sessionId,
        path: currentPath!,
      }),
    retry: false,
  });
  const entries = directoryQuery.data?.entries ?? [];
  const selectedEntry = entries.find((entry) => entry.path === selectedPath) ?? null;

  const nameMutation = useMutation({
    mutationFn: async () => {
      const name = validateRemoteName(nameValue, t("ssh.sftp.invalidName"));
      const basePath = currentPath ?? "/";
      if (nameAction === "mkdir") {
        await createSftpDirectory({
          workspaceId: session.workspaceId,
          sessionId: session.sessionId,
          path: joinRemotePath(basePath, name),
        });
        return null;
      }
      if (nameAction === "rename" && selectedEntry) {
        await renameSftpPath({
          workspaceId: session.workspaceId,
          sessionId: session.sessionId,
          oldPath: selectedEntry.path,
          newPath: joinRemotePath(basePath, name),
        });
        return null;
      }
      if (nameAction === "upload" && localUploadPath) {
        const existing = entries.find((entry) => entry.name === name);
        if (existing && existing.kind !== "file") {
          throw new Error(t("ssh.sftp.uploadCannotReplaceNonFile"));
        }
        return uploadSftpFile({
          workspaceId: session.workspaceId,
          sessionId: session.sessionId,
          localPath: localUploadPath,
          remotePath: joinRemotePath(basePath, name),
          overwrite: Boolean(existing),
        });
      }
      throw new Error(t("ssh.sftp.invalidOperation"));
    },
    onSuccess: (transfer) => {
      if (transfer) upsertTransfer(transfer);
      closeNameDialog();
      void refreshDirectory();
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (entry: SftpFileEntry) =>
      deleteSftpPath({
        workspaceId: session.workspaceId,
        sessionId: session.sessionId,
        path: entry.path,
        isDirectory: entry.kind === "directory",
      }),
    onSuccess: () => {
      setDeleteTarget(null);
      setSelectedPath(session.sessionId, session.connectionId, null);
      void refreshDirectory();
    },
    onError: (error) => handleError(error, { key: "ssh.sftp.deleteFailed" }),
  });

  const cancelMutation = useMutation({
    mutationFn: (transfer: SftpTransferState) =>
      cancelSftpTransfer({
        workspaceId: session.workspaceId,
        transferId: transfer.transferId,
      }),
    onSuccess: upsertTransfer,
    onError: (error) => handleError(error, { key: "ssh.sftp.cancelFailed" }),
  });

  function navigate(path: string) {
    try {
      const normalized = normalizeClientPath(path, t("ssh.sftp.invalidAbsolutePath"));
      setPathDraft({ dirty: false, sessionId: session.sessionId, value: normalized });
      setPanelPath(session.sessionId, session.connectionId, normalized);
    } catch (error) {
      handleError(error);
    }
  }

  async function refreshDirectory() {
    await queryClient.invalidateQueries({
      queryKey: ["ssh-sftp-directory", session.workspaceId, session.sessionId],
    });
  }

  function closeNameDialog() {
    setNameAction(null);
    setNameValue("");
    setLocalUploadPath(null);
    nameMutation.reset();
  }

  async function chooseUpload() {
    if (!isTauriRuntime()) {
      handleError(new Error(t("ssh.sftp.nativeDialogUnavailable")));
      return;
    }
    try {
      const selection = await openFileDialog({ directory: false, multiple: false });
      if (typeof selection !== "string") return;
      setLocalUploadPath(selection);
      setNameValue(localFileName(selection));
      setNameAction("upload");
    } catch (error) {
      handleError(error, { key: "ssh.sftp.selectUploadFailed" });
    }
  }

  async function chooseDownload() {
    if (!selectedEntry || selectedEntry.kind !== "file") return;
    if (!isTauriRuntime()) {
      handleError(new Error(t("ssh.sftp.nativeDialogUnavailable")));
      return;
    }
    try {
      const target = await saveFileDialog({ defaultPath: selectedEntry.name });
      if (!target) return;
      const transfer = await downloadSftpFile({
        workspaceId: session.workspaceId,
        sessionId: session.sessionId,
        localPath: target,
        remotePath: selectedEntry.path,
        overwrite: true,
      });
      upsertTransfer(transfer);
    } catch (error) {
      handleError(error, { key: "ssh.sftp.downloadFailed" });
    }
  }

  const uploadExisting =
    nameAction === "upload"
      ? entries.find((entry) => entry.name === nameValue.trim())
      : undefined;
  const uploadConflict = uploadExisting?.kind === "file";
  const uploadBlocked = Boolean(uploadExisting && uploadExisting.kind !== "file");
  const nameDialogTitle = nameAction ? t(`ssh.sftp.dialog.${nameAction}.title`) : "";
  const nameDialogConfirm = nameAction
    ? uploadConflict
      ? t("ssh.sftp.dialog.upload.replace")
      : t(`ssh.sftp.dialog.${nameAction}.confirm`)
    : "";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <SftpToolbar
        canRefresh={directoryQuery.isSuccess}
        connected={connected}
        currentPath={currentPath}
        endpoint={`${session.username}@${session.host}`}
        onClose={onClose}
        onDelete={() => selectedEntry && setDeleteTarget(selectedEntry)}
        onDownload={() => void chooseDownload()}
        onNewFolder={() => {
          setNameValue("");
          setNameAction("mkdir");
        }}
        onParent={() => currentPath && navigate(parentRemotePath(currentPath))}
        onPathChange={(value) =>
          setPathDraft({ dirty: true, sessionId: session.sessionId, value })
        }
        onPathSubmit={() => navigate(pathValue)}
        onRefresh={() => void directoryQuery.refetch()}
        onRename={() => {
          if (!selectedEntry) return;
          setNameValue(selectedEntry.name);
          setNameAction("rename");
        }}
        onUpload={() => void chooseUpload()}
        opening={openQuery.isPending}
        pathValue={pathValue}
        refreshing={directoryQuery.isFetching}
        selectedEntry={selectedEntry}
      />
      {!connected ? (
        <ErrorState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.sftp.disconnected")}
        </ErrorState>
      ) : (
        <SftpFileList
          entries={entries}
          error={openQuery.error ?? directoryQuery.error}
          loading={openQuery.isPending || (openQuery.isSuccess && directoryQuery.isPending)}
          onActivate={(entry) => {
            if (entry.kind === "directory") navigate(entry.path);
          }}
          onRetry={() => {
            if (openQuery.isError) void openQuery.refetch();
            else void directoryQuery.refetch();
          }}
          onSelect={(entry) =>
            setSelectedPath(session.sessionId, session.connectionId, entry.path)
          }
          selectedPath={selectedPath}
        />
      )}
      <SftpTransferList
        onCancel={(transfer) => cancelMutation.mutate(transfer)}
        onReveal={(transfer) => {
          if (isTauriRuntime()) {
            void revealItemInDir(transfer.localPath).catch((error) =>
              handleError(error, { key: "ssh.sftp.revealFailed" }),
            );
          }
        }}
        transfers={transfers}
      />
      <SftpNameDialog
        confirmLabel={nameDialogConfirm}
        confirmDisabled={uploadBlocked}
        error={mutationError(nameMutation.error)}
        label={t("ssh.sftp.dialog.name")}
        onConfirm={() => nameMutation.mutate()}
        onOpenChange={(open) => !open && closeNameDialog()}
        onValueChange={setNameValue}
        open={nameAction !== null}
        pending={nameMutation.isPending}
        title={nameDialogTitle}
        value={nameValue}
        warning={
          uploadBlocked
            ? t("ssh.sftp.uploadCannotReplaceNonFile")
            : uploadConflict
              ? t("ssh.sftp.dialog.upload.conflict")
              : null
        }
      />
      <ConfirmDialog
        confirmLabel={t("ssh.sftp.dialog.delete.confirm")}
        description={
          deleteTarget
            ? t("ssh.sftp.dialog.delete.description", { name: deleteTarget.name })
            : ""
        }
        onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget)}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        open={deleteTarget !== null}
        pending={deleteMutation.isPending}
        title={t("ssh.sftp.dialog.delete.title")}
      />
    </div>
  );
}

function normalizeClientPath(path: string, errorMessage: string) {
  if (!path.trim().startsWith("/")) throw new Error(errorMessage);
  const parts: string[] = [];
  for (const part of path.trim().split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return `/${parts.join("/")}`;
}

function parentRemotePath(path: string) {
  const parts = path.split("/").filter(Boolean);
  parts.pop();
  return `/${parts.join("/")}`;
}

function joinRemotePath(parent: string, name: string) {
  return `${parent === "/" ? "" : parent}/${name}`;
}

function validateRemoteName(name: string, errorMessage: string) {
  const value = name.trim();
  if (!value || value === "." || value === ".." || value.includes("/") || value.includes("\0")) {
    throw new Error(errorMessage);
  }
  return value;
}

function localFileName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function mutationError(error: unknown) {
  return error instanceof Error ? error.message : error ? String(error) : null;
}

function isTauriRuntime() {
  return (
    typeof window !== "undefined" &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}
