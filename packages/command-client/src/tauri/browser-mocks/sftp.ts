import type {
  SftpDeleteInput,
  SftpFileEntry,
  SftpPathInput,
  SftpRenameInput,
  SftpSessionInput,
  SftpTransferInput,
  SftpTransferState,
} from "../../types";
import { mockStore } from "./state";
import { UNHANDLED, type MockResult } from "./types";

const mockSftpEntries = new Map<string, SftpFileEntry[]>();
const mockSftpTransfers: SftpTransferState[] = [];

function sftpEntries(sessionId: string) {
  let entries = mockSftpEntries.get(sessionId);
  if (!entries) {
    const now = new Date().toISOString();
    entries = [
      {
        name: ".config",
        path: "/home/demo/.config",
        kind: "directory",
        size: 0,
        modifiedAt: now,
        permissions: "rwxr-xr-x",
        linkTarget: null,
      },
      {
        name: "projects",
        path: "/home/demo/projects",
        kind: "directory",
        size: 0,
        modifiedAt: now,
        permissions: "rwxr-xr-x",
        linkTarget: null,
      },
      {
        name: "README.md",
        path: "/home/demo/README.md",
        kind: "file",
        size: 2048,
        modifiedAt: now,
        permissions: "rw-r--r--",
        linkTarget: null,
      },
      {
        name: "示例.txt",
        path: "/home/demo/示例.txt",
        kind: "file",
        size: 512,
        modifiedAt: now,
        permissions: "rw-r--r--",
        linkTarget: null,
      },
    ];
    mockSftpEntries.set(sessionId, entries);
  }
  return entries;
}

function connectedSession(input: SftpSessionInput) {
  const session = mockStore.sshSessions.find(
    (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
  );
  if (!session || session.status !== "connected") {
    throw new Error("ssh session is not connected");
  }
  return session;
}

function mockTransfer(input: SftpTransferInput, direction: "upload" | "download") {
  const session = connectedSession(input);
  const now = new Date().toISOString();
  const transfer: SftpTransferState = {
    transferId: crypto.randomUUID(),
    workspaceId: input.workspaceId,
    sessionId: input.sessionId,
    connectionId: session.connectionId,
    direction,
    localPath: input.localPath,
    remotePath: input.remotePath,
    transferredBytes: 4096,
    totalBytes: 4096,
    bytesPerSecond: 1024 * 1024,
    status: "success",
    error: null,
    startedAt: now,
    finishedAt: now,
  };
  mockSftpTransfers.unshift(transfer);
  return transfer;
}

export function handleSftpMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  for (const handler of [handleSftpBrowseMock, handleSftpMutationMock, handleSftpTransferMock]) {
    const result = handler<T>(command, args);
    if (result !== UNHANDLED) return result;
  }
  return UNHANDLED;
}

function handleSftpBrowseMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "ssh_sftp_open") {
    const input = args?.input as SftpSessionInput;
    const session = connectedSession(input);
    sftpEntries(input.sessionId);
    return { ...input, connectionId: session.connectionId, homePath: "/home/demo" } as T;
  }

  if (command === "ssh_sftp_list_directory") {
    const input = args?.input as SftpPathInput;
    const session = connectedSession(input);
    const path = input.path.replace(/\/+$/, "") || "/";
    const entries = sftpEntries(input.sessionId).filter((entry) => {
      const parent = entry.path.slice(0, Math.max(1, entry.path.lastIndexOf("/"))) || "/";
      return parent === path;
    });
    return { ...input, connectionId: session.connectionId, path, entries } as T;
  }

  if (command === "ssh_sftp_stat") {
    const input = args?.input as SftpPathInput;
    const entry = sftpEntries(input.sessionId).find((item) => item.path === input.path);
    if (!entry) throw new Error("remote path not found");
    return entry as T;
  }

  return UNHANDLED;
}

function handleSftpMutationMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "ssh_sftp_create_directory") {
    const input = args?.input as SftpPathInput;
    const nameParts = input.path.split("/").filter(Boolean);
    sftpEntries(input.sessionId).push({
      name: nameParts[nameParts.length - 1] ?? "folder",
      path: input.path,
      kind: "directory",
      size: 0,
      modifiedAt: new Date().toISOString(),
      permissions: "rwxr-xr-x",
      linkTarget: null,
    });
    return undefined as T;
  }

  if (command === "ssh_sftp_rename") {
    const input = args?.input as SftpRenameInput;
    const entry = sftpEntries(input.sessionId).find((item) => item.path === input.oldPath);
    if (!entry) throw new Error("remote path not found");
    entry.path = input.newPath;
    const nameParts = input.newPath.split("/").filter(Boolean);
    entry.name = nameParts[nameParts.length - 1] ?? entry.name;
    return undefined as T;
  }

  if (command === "ssh_sftp_delete") {
    const input = args?.input as SftpDeleteInput;
    mockSftpEntries.set(
      input.sessionId,
      sftpEntries(input.sessionId).filter((entry) => entry.path !== input.path),
    );
    return undefined as T;
  }

  return UNHANDLED;
}

function handleSftpTransferMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "ssh_sftp_download" || command === "ssh_sftp_upload") {
    return mockTransfer(
      args?.input as SftpTransferInput,
      command === "ssh_sftp_download" ? "download" : "upload",
    ) as T;
  }

  if (command === "ssh_sftp_cancel_transfer") {
    const input = args?.input as { workspaceId: string; transferId: string };
    const transfer = mockSftpTransfers.find(
      (item) => item.workspaceId === input.workspaceId && item.transferId === input.transferId,
    );
    if (!transfer) throw new Error("SFTP transfer not found");
    transfer.status = "cancelled";
    transfer.finishedAt = new Date().toISOString();
    return transfer as T;
  }

  if (command === "ssh_sftp_transfers_list") {
    const input = args?.input as SftpSessionInput;
    return mockSftpTransfers.filter(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    ) as T;
  }

  return UNHANDLED;
}
