import { useEffect, useMemo, useRef, useState } from "react";
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
import { useSftpNativeDragDrop } from "../hooks/useSftpNativeDragDrop";
import { useSftpStore } from "../model/sftp-state";
import { SftpFileList } from "./SftpFileList";
import { SftpNameDialog } from "./SftpNameDialog";
import { SftpTransferList } from "./SftpTransferList";
import { SftpToolbar } from "./SftpToolbar";

type NameAction = "mkdir" | "rename" | "upload";
const EMPTY_TRANSFERS: SftpTransferState[] = [];
const EMPTY_SELECTED_PATHS: string[] = [];

function isActiveTransfer(transfer: SftpTransferState) {
  return transfer.status === "pending" || transfer.status === "running";
}

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
  const setSelectedPaths = useSftpStore((state) => state.setSelectedPaths);
  const upsertTransfer = useSftpStore((state) => state.upsertTransfer);
  const listRef = useRef<HTMLDivElement | null>(null);
  const [pathDraft, setPathDraft] = useState<{
    dirty: boolean;
    sessionId: string;
    value: string;
  }>({ dirty: false, sessionId: session.sessionId, value: "" });
  const [nameAction, setNameAction] = useState<NameAction | null>(null);
  const [nameValue, setNameValue] = useState("");
  const [localUploadPath, setLocalUploadPath] = useState<string | null>(null);
  const [deleteTargets, setDeleteTargets] = useState<SftpFileEntry[]>([]);
  const connected = session.status === "connected";
  const hasActiveTransfer = transfers.some(isActiveTransfer);
  const seenUploadSuccessRef = useRef(new Set<string>());

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
  const selectedPaths =
    tab?.connectionId === session.connectionId
      ? (tab.selectedPaths ?? EMPTY_SELECTED_PATHS)
      : EMPTY_SELECTED_PATHS;
  const pathValue =
    pathDraft.sessionId === session.sessionId && pathDraft.dirty
      ? pathDraft.value
      : currentPath ?? "";

  const directoryQuery = useQuery({
    // Keep directory listing off the shared SFTP channel while a transfer runs;
    // russh-sftp serializes requests, so a refresh at upload start stalls at 0%.
    enabled:
      connected && openQuery.isSuccess && Boolean(currentPath) && !hasActiveTransfer,
    queryKey: ["ssh-sftp-directory", session.workspaceId, session.sessionId, currentPath],
    queryFn: () =>
      listSftpDirectory({
        workspaceId: session.workspaceId,
        sessionId: session.sessionId,
        path: currentPath!,
      }),
    refetchOnWindowFocus: false,
    retry: false,
    staleTime: 10_000,
  });
  const entries = directoryQuery.data?.entries ?? [];
  const selectedEntry = entries.find((entry) => entry.path === selectedPath) ?? null;
  const selectedEntries = useMemo(
    () => entries.filter((entry) => selectedPaths.includes(entry.path)),
    [entries, selectedPaths],
  );

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
      if (transfer) {
        // Upload keeps using the SFTP channel; refresh after it finishes.
        upsertTransfer(transfer);
        closeNameDialog();
        return;
      }
      closeNameDialog();
      void refreshDirectory();
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (targets: SftpFileEntry[]) => {
      for (const entry of targets) {
        await deleteSftpPath({
          workspaceId: session.workspaceId,
          sessionId: session.sessionId,
          path: entry.path,
          isDirectory: entry.kind === "directory",
        });
      }
    },
    onSuccess: () => {
      setDeleteTargets([]);
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
    onSuccess: (state) => {
      // Cancel returns the authoritative backend snapshot. If the transfer already
      // finished while the UI was stale, this just resyncs (often as success).
      upsertTransfer(state);
    },
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

  useEffect(() => {
    let shouldRefresh = false;
    for (const transfer of transfers) {
      if (
        transfer.direction === "upload" &&
        transfer.status === "success" &&
        !seenUploadSuccessRef.current.has(transfer.transferId)
      ) {
        seenUploadSuccessRef.current.add(transfer.transferId);
        shouldRefresh = true;
      }
    }
    if (!shouldRefresh) return;
    void queryClient.invalidateQueries({
      queryKey: ["ssh-sftp-directory", session.workspaceId, session.sessionId],
    });
  }, [transfers, queryClient, session.workspaceId, session.sessionId]);

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

  async function chooseDownload(entry: SftpFileEntry | null = selectedEntry) {
    if (!entry || entry.kind !== "file") return;
    if (!isTauriRuntime()) {
      handleError(new Error(t("ssh.sftp.nativeDialogUnavailable")));
      return;
    }
    try {
      const target = await saveFileDialog({ defaultPath: entry.name });
      if (!target) return;
      const transfer = await downloadSftpFile({
        workspaceId: session.workspaceId,
        sessionId: session.sessionId,
        localPath: target,
        remotePath: entry.path,
        overwrite: true,
      });
      upsertTransfer(transfer);
    } catch (error) {
      handleError(error, { key: "ssh.sftp.downloadFailed" });
    }
  }

  const dropActive = useSftpNativeDragDrop({
    connected,
    currentPath,
    entries,
    listRef,
    onError: handleError,
    onTransfer: upsertTransfer,
    sessionId: session.sessionId,
    workspaceId: session.workspaceId,
  });

  function selectEntry(entry: SftpFileEntry) {
    setSelectedPath(session.sessionId, session.connectionId, entry.path);
  }

  function toggleSelect(entry: SftpFileEntry) {
    const exists = selectedPaths.includes(entry.path);
    const next = exists
      ? selectedPaths.filter((path) => path !== entry.path)
      : [...selectedPaths, entry.path];
    setSelectedPaths(
      session.sessionId,
      session.connectionId,
      next,
      exists ? (next[next.length - 1] ?? null) : entry.path,
    );
  }

  function selectRange(entry: SftpFileEntry) {
    const anchorPath = selectedPath;
    const anchorIndex = anchorPath
      ? entries.findIndex((item) => item.path === anchorPath)
      : -1;
    const targetIndex = entries.findIndex((item) => item.path === entry.path);
    if (anchorIndex < 0 || targetIndex < 0) {
      selectEntry(entry);
      return;
    }
    const [start, end] =
      anchorIndex < targetIndex ? [anchorIndex, targetIndex] : [targetIndex, anchorIndex];
    const paths = entries.slice(start, end + 1).map((item) => item.path);
    setSelectedPaths(session.sessionId, session.connectionId, paths, entry.path);
  }

  function beginRename(entry: SftpFileEntry) {
    setSelectedPath(session.sessionId, session.connectionId, entry.path);
    setNameValue(entry.name);
    setNameAction("rename");
  }

  function beginDelete(targets: SftpFileEntry[]) {
    if (targets.length === 0) return;
    setDeleteTargets(targets);
  }

  async function uploadHere(entry: SftpFileEntry) {
    if (entry.kind !== "directory") return;
    navigate(entry.path);
    await chooseUpload();
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
  const canUpload = connected && Boolean(currentPath);
  const canGoParent = connected && Boolean(currentPath) && currentPath !== "/";
  const canRefresh = connected && directoryQuery.isSuccess && !hasActiveTransfer;
  const deleteMany = deleteTargets.length > 1;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <SftpToolbar
        canRefresh={canRefresh}
        connected={connected}
        currentPath={currentPath}
        endpoint={`${session.username}@${session.host}`}
        onClose={onClose}
        onDelete={() => beginDelete(selectedEntries)}
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
          beginRename(selectedEntry);
        }}
        onUpload={() => void chooseUpload()}
        opening={openQuery.isPending}
        pathValue={pathValue}
        refreshing={directoryQuery.isFetching}
        selectedEntry={selectedEntry}
        selectedCount={selectedEntries.length}
      />
      {!connected ? (
        <ErrorState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.sftp.disconnected")}
        </ErrorState>
      ) : (
        <div
          className={
            dropActive
              ? "relative flex min-h-0 flex-1 flex-col outline outline-2 outline-[var(--u-color-primary)]"
              : "relative flex min-h-0 flex-1 flex-col"
          }
          ref={listRef}
        >
          <SftpFileList
            actions={{
              canGoParent,
              canRefresh,
              canUpload,
              onCopyPath: (entry) => void navigator.clipboard?.writeText(entry.path),
              onDelete: (entry) => beginDelete([entry]),
              onDownload: (entry) => void chooseDownload(entry),
              onNewFolder: () => {
                setNameValue("");
                setNameAction("mkdir");
              },
              onOpen: (entry) => {
                if (entry.kind === "directory") navigate(entry.path);
              },
              onParent: () => currentPath && navigate(parentRemotePath(currentPath)),
              onRefresh: () => void directoryQuery.refetch(),
              onRename: beginRename,
              onUpload: () => void chooseUpload(),
              onUploadHere: (entry) => void uploadHere(entry),
            }}
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
            onSelect={selectEntry}
            onSelectRange={selectRange}
            onToggleSelect={toggleSelect}
            selectedPaths={selectedPaths}
          />
        </div>
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
          deleteMany
            ? t("ssh.sftp.dialog.delete.descriptionMany", { count: deleteTargets.length })
            : deleteTargets[0]
              ? t("ssh.sftp.dialog.delete.description", { name: deleteTargets[0].name })
              : ""
        }
        onConfirm={() => deleteTargets.length > 0 && deleteMutation.mutate(deleteTargets)}
        onOpenChange={(open) => !open && setDeleteTargets([])}
        open={deleteTargets.length > 0}
        pending={deleteMutation.isPending}
        title={
          deleteMany
            ? t("ssh.sftp.dialog.delete.titleMany")
            : t("ssh.sftp.dialog.delete.title")
        }
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
