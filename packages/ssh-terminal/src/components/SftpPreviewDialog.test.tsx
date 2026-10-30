// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SftpFileEntry, SftpTextPreview, SshSessionSummary } from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { StrictMode } from "react";
import { SftpPreviewDialog } from "./SftpPreviewDialog";

const preview = vi.hoisted(() => vi.fn());
vi.mock("@unfour/command-client", () => ({ previewSftpText: preview }));
const session: SshSessionSummary = {
  workspaceId: "workspace-a", sessionId: "session-a", connectionId: "connection-a",
  status: "connected", reconnectAttempt: 0, authKind: "password", host: "fixture",
  username: "demo", cols: 80, rows: 24, createdAt: "", updatedAt: "",
};
const entry: SftpFileEntry = {
  kind: "file", name: "settings.env", path: "/settings.env", size: 8, modifiedAt: null,
};
function deferred() {
  let resolve!: (value: SftpTextPreview) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<SftpTextPreview>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
beforeEach(() => { preview.mockReset(); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it("issues one read when Strict Mode replays effects", async () => {
  preview.mockResolvedValue({ content: "once", size: 4 });
  render(<StrictMode><SftpPreviewDialog entry={entry} session={session} onClose={vi.fn()} /></StrictMode>);
  await screen.findByText("once");
  expect(preview).toHaveBeenCalledOnce();
});

it.each(["log", "txt", "json", "yaml", "yml", "xml", "conf", "ini", "sh", "sql", "env"])(
  "loads %s text on demand and copies only on explicit action", async (extension) => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    const content = "hello 世界\n<script>plain text</script>";
    preview.mockResolvedValue({ content, size: 48 });
    const file = { ...entry, name: `test.${extension}`, path: `/test.${extension}` };
    render(<SftpPreviewDialog entry={file} session={session} onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByLabelText("Remote text content")).toHaveTextContent("hello 世界"));
    expect(screen.getByLabelText("Remote text content").textContent).toBe(content);
    expect(preview).toHaveBeenCalledExactlyOnceWith({ workspaceId: "workspace-a", sessionId: "session-a", path: file.path });
    expect(writeText).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Copy content" }));
    expect(writeText).toHaveBeenCalledWith(content);
    await screen.findByText("Copied");
  },
);

it.each([
  ["TOO_LARGE", "exceeds the 1 MiB"], ["BINARY", "binary data"],
  ["ENCODING", "not valid UTF-8"], ["PERMISSION_DENIED", "Permission denied"],
  ["DISCONNECTED", "connection is unavailable"], ["TIMEOUT", "timed out"],
  ["NOT_FILE", "Only regular files"], ["NOT_FOUND", "no longer exists"],
  ["READ_FAILED", "Could not read"],
])("shows a safe localized %s error", async (code, message) => {
  preview.mockRejectedValue({ code: `SFTP_PREVIEW_${code}`, message: "remote secret detail" });
  render(<SftpPreviewDialog entry={entry} session={session} onClose={vi.fn()} />);
  expect(await screen.findByRole("alert")).toHaveTextContent(message);
  expect(screen.queryByText("remote secret detail")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Copy content" })).toBeDisabled();
});

it("ignores a late result after close and reopening the same file", async () => {
  const old = deferred();
  preview.mockReturnValueOnce(old.promise).mockResolvedValueOnce({ content: "new content", size: 11 });
  const onClose = vi.fn();
  const first = render(<SftpPreviewDialog entry={entry} session={session} onClose={onClose} />);
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(onClose).toHaveBeenCalledOnce();
  first.unmount();
  render(<SftpPreviewDialog entry={entry} session={session} onClose={onClose} />);
  await screen.findByText("new content");
  await act(async () => old.resolve({ content: "old secret", size: 10 }));
  expect(screen.queryByText("old secret")).not.toBeInTheDocument();
});

it.each(["workspaceId", "sessionId", "connectionId"] as const)("isolates results after %s changes", async (field) => {
  const old = deferred();
  preview.mockReturnValueOnce(old.promise).mockResolvedValueOnce({ content: "current content", size: 15 });
  const view = render(<SftpPreviewDialog entry={entry} session={session} onClose={vi.fn()} />);
  view.rerender(<SftpPreviewDialog entry={entry} session={{ ...session, [field]: "new-scope" }} onClose={vi.fn()} />);
  await screen.findByText("current content");
  await act(async () => old.reject({ code: "SFTP_PREVIEW_PERMISSION_DENIED" }));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByText("current content")).toBeInTheDocument();
});

it("clears content on disconnect and ignores the pending response", async () => {
  const old = deferred();
  preview.mockReturnValue(old.promise);
  const view = render(<SftpPreviewDialog entry={entry} session={session} onClose={vi.fn()} />);
  view.rerender(<SftpPreviewDialog entry={entry} session={{ ...session, status: "disconnected" }} onClose={vi.fn()} />);
  expect(screen.getByRole("alert")).toHaveTextContent("connection is unavailable");
  await act(async () => old.resolve({ content: "stale", size: 5 }));
  expect(screen.queryByText("stale")).not.toBeInTheDocument();
  expect(preview).toHaveBeenCalledOnce();
});

it("uses Chinese messages and reports clipboard failure without logging content", async () => {
  vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn().mockRejectedValue(new Error("secret")) } });
  preview.mockResolvedValue({ content: "文本", size: 6 });
  render(<I18nProvider initialLocale="zh-CN" storageKey="sftp-preview-test">
    <SftpPreviewDialog entry={entry} session={session} onClose={vi.fn()} />
  </I18nProvider>);
  await screen.findByText("文本");
  fireEvent.click(screen.getByRole("button", { name: "复制内容" }));
  expect(await screen.findByRole("status")).toHaveTextContent("复制失败");
});
