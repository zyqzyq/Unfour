// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FeedbackProvider, I18nProvider } from "@unfour/ui";
import type { SftpTransferState, SshSessionSummary } from "@unfour/command-client";
import { SftpPanel } from "./SftpPanel";
import { useSftpStore } from "../model/sftp-state";

const mocks = vi.hoisted(() => ({
  open: vi.fn(),
  list: vi.fn(),
  mkdir: vi.fn(),
  rename: vi.fn(),
  remove: vi.fn(),
  download: vi.fn(),
  upload: vi.fn(),
  saveDialog: vi.fn(),
  openDialog: vi.fn(),
  preview: vi.fn(),
}));

vi.mock("@unfour/command-client", () => ({
  openSftp: mocks.open,
  previewSftpText: mocks.preview,
  listSftpDirectory: mocks.list,
  createSftpDirectory: mocks.mkdir,
  renameSftpPath: mocks.rename,
  deleteSftpPath: mocks.remove,
  downloadSftpFile: mocks.download,
  uploadSftpFile: mocks.upload,
  cancelSftpTransfer: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mocks.openDialog(...args),
  save: (...args: unknown[]) => mocks.saveDialog(...args),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));

const session: SshSessionSummary = {
  sessionId: "session-1",
  workspaceId: "workspace-1",
  connectionId: "connection-1",
  status: "connected",
  reconnectAttempt: 0,
  authKind: "password",
  host: "fixture.example",
  username: "demo",
  cols: 80,
  rows: 24,
  createdAt: "2026-10-19T00:00:00.000Z",
  updatedAt: "2026-10-19T00:00:00.000Z",
};

const entry = {
  name: "notes.txt",
  path: "/home/demo/notes.txt",
  kind: "file" as const,
  size: 4,
  modifiedAt: null,
  permissions: null,
  linkTarget: null,
};

function transfer(id: string, direction: "download" | "upload"): SftpTransferState {
  return {
    transferId: id,
    workspaceId: session.workspaceId,
    sessionId: session.sessionId,
    connectionId: session.connectionId,
    direction,
    localPath: `C:/temp/${id}.txt`,
    remotePath: "/home/demo/notes.txt",
    transferredBytes: 0,
    totalBytes: 4,
    bytesPerSecond: 0,
    status: "pending",
    error: null,
    startedAt: "2026-10-19T00:00:00.000Z",
    finishedAt: null,
    warnings: ["ACTIVITY_RECORD_FAILED"],
  };
}

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <I18nProvider initialLocale="en" storageKey="sftp-activity-test">
        <FeedbackProvider>
          <SftpPanel onClose={vi.fn()} session={session} />
        </FeedbackProvider>
      </I18nProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__TAURI_INTERNALS__", {});
  useSftpStore.setState({ tabs: {}, transfers: {} });
  mocks.open.mockResolvedValue({
    workspaceId: session.workspaceId,
    sessionId: session.sessionId,
    connectionId: session.connectionId,
    homePath: "/home/demo",
  });
  mocks.list.mockResolvedValue({
    workspaceId: session.workspaceId,
    sessionId: session.sessionId,
    connectionId: session.connectionId,
    path: "/home/demo",
    entries: [entry],
  });
  mocks.mkdir.mockResolvedValue({ warnings: ["ACTIVITY_RECORD_FAILED"] });
  mocks.rename.mockResolvedValue({ warnings: ["ACTIVITY_RECORD_FAILED"] });
  mocks.remove.mockResolvedValue({ warnings: ["ACTIVITY_RECORD_FAILED"] });
  mocks.download.mockResolvedValue(transfer("transfer-download", "download"));
  mocks.upload.mockResolvedValue(transfer("transfer-upload", "upload"));
  mocks.saveDialog.mockResolvedValue("C:/temp/notes.txt");
  mocks.openDialog.mockResolvedValue("C:/temp/local-secret.txt");
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("keeps SFTP success and transfer ids when activity logging fails", async () => {
  renderPanel();
  expect(await screen.findByRole("row", { name: /notes\.txt/ })).toBeVisible();

  fireEvent.click(screen.getByRole("button", { name: "Create remote folder" }));
  const createDialog = await screen.findByRole("dialog", { name: /Create remote folder/ });
  fireEvent.change(within(createDialog).getByRole("textbox"), { target: { value: "audit-dir" } });
  fireEvent.click(within(createDialog).getByRole("button", { name: "Create" }));
  await waitFor(() => expect(mocks.mkdir).toHaveBeenCalledTimes(1));
  expect(mocks.mkdir).toHaveBeenCalledWith({
    workspaceId: session.workspaceId,
    sessionId: session.sessionId,
    path: "/home/demo/audit-dir",
  });
  expect(await screen.findByText(/activity log could not be saved/i)).toBeVisible();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await waitFor(() => expect(mocks.list.mock.calls.length).toBeGreaterThan(1));

  fireEvent.click(screen.getByRole("row", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByRole("button", { name: "Rename selected remote item" }));
  const renameDialog = await screen.findByRole("dialog", { name: /Rename remote item/ });
  fireEvent.change(within(renameDialog).getByRole("textbox"), { target: { value: "renamed.txt" } });
  fireEvent.click(within(renameDialog).getByRole("button", { name: "Rename" }));
  await waitFor(() => expect(mocks.rename).toHaveBeenCalledTimes(1));
  expect(mocks.rename).toHaveBeenCalledWith(expect.objectContaining({
    oldPath: "/home/demo/notes.txt",
    newPath: "/home/demo/renamed.txt",
  }));

  fireEvent.click(screen.getByRole("row", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByRole("button", { name: "Delete selected remote item" }));
  fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
  await waitFor(() => expect(mocks.remove).toHaveBeenCalledTimes(1));
  expect(mocks.remove).toHaveBeenCalledWith(expect.objectContaining({
    path: "/home/demo/notes.txt",
    isDirectory: false,
  }));

  fireEvent.click(screen.getByRole("row", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByRole("button", { name: "Download selected file" }));
  await waitFor(() => expect(mocks.download).toHaveBeenCalledTimes(1));
  expect(useSftpStore.getState().transfers[session.sessionId]?.map((item) => item.transferId))
    .toContain("transfer-download");

  fireEvent.click(screen.getByRole("button", { name: "Upload local file" }));
  const uploadDialog = await screen.findByRole("dialog", { name: /Upload file/ });
  fireEvent.click(within(uploadDialog).getByRole("button", { name: "Upload" }));
  await waitFor(() => expect(mocks.upload).toHaveBeenCalledTimes(1));
  expect(useSftpStore.getState().transfers[session.sessionId]?.map((item) => item.transferId))
    .toEqual(expect.arrayContaining(["transfer-download", "transfer-upload"]));

  const notices = screen.getAllByText(/activity log could not be saved/i);
  expect(notices).toHaveLength(5);
  for (const notice of notices) {
    expect(notice.textContent).not.toMatch(/notes\.txt|audit sink|local-secret/i);
  }
  expect(mocks.mkdir).toHaveBeenCalledTimes(1);
  expect(mocks.rename).toHaveBeenCalledTimes(1);
  expect(mocks.remove).toHaveBeenCalledTimes(1);
  expect(mocks.download).toHaveBeenCalledTimes(1);
  expect(mocks.upload).toHaveBeenCalledTimes(1);
});


it("previews on demand without transfer, mutation, local dialog, or persistent content", async () => {
  mocks.preview.mockResolvedValue({ content: "private preview fixture", size: 23 });
  const { unmount } = renderPanel();
  const row = await screen.findByRole("row", { name: /notes\.txt/ });
  expect(mocks.preview).not.toHaveBeenCalled();
  fireEvent.doubleClick(row);
  expect(mocks.preview).not.toHaveBeenCalled();
  fireEvent.contextMenu(row);
  fireEvent.click(screen.getByRole("menuitem", { name: "Preview" }));
  await screen.findByText("private preview fixture");
  expect(mocks.preview).toHaveBeenCalledExactlyOnceWith({
    workspaceId: session.workspaceId, sessionId: session.sessionId, path: entry.path,
  });
  for (const action of [mocks.upload, mocks.download, mocks.mkdir, mocks.rename, mocks.remove, mocks.saveDialog, mocks.openDialog]) {
    expect(action).not.toHaveBeenCalled();
  }
  expect(JSON.stringify(useSftpStore.getState())).not.toContain("private preview fixture");
  expect(JSON.stringify(window.localStorage)).not.toContain("private preview fixture");
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.queryByText("private preview fixture")).not.toBeInTheDocument();
  unmount();
});
