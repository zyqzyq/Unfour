// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { getCurrentWebview } from "@tauri-apps/api/webview";
import { useSftpNativeDragDrop } from "./useSftpNativeDragDrop";

type DragListener = Parameters<ReturnType<typeof getCurrentWebview>["onDragDropEvent"]>[0];
const mocks = vi.hoisted(() => ({ listen: vi.fn(), scale: vi.fn(), upload: vi.fn(), stop: vi.fn() }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: mocks.listen }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ scaleFactor: mocks.scale }) }));
vi.mock("@unfour/command-client", () => ({ uploadSftpFile: mocks.upload }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__TAURI_INTERNALS__", {});
  mocks.listen.mockResolvedValue(mocks.stop);
  mocks.scale.mockResolvedValue(1);
  mocks.upload.mockResolvedValue({ id: "transfer" });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

function options() {
  const list = document.createElement("div");
  list.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  return { connected: true, currentPath: "/old", entries: [], listRef: { current: list },
    onError: vi.fn(), onTransfer: vi.fn(), sessionId: "session", workspaceId: "workspace" };
}

function drop(paths = ["C:/file.txt"]): Parameters<DragListener>[0] {
  return { id: 1, event: "tauri://drag-drop", payload: { type: "drop", paths, position: new PhysicalPosition(10, 10) } };
}

it("disposes a native registration that resolves after unmount", async () => {
  let ready!: (stop: () => void) => void;
  mocks.listen.mockReturnValueOnce(new Promise((resolve) => { ready = resolve; }));
  const { unmount } = renderHook(() => useSftpNativeDragDrop(options()));
  unmount();
  await act(async () => { ready(mocks.stop); });
  expect(mocks.stop).toHaveBeenCalledTimes(1);
});

it("uses the latest committed destination without registering another listener", async () => {
  const input = options();
  const { rerender, unmount } = renderHook((props) => useSftpNativeDragDrop(props), { initialProps: input });
  await act(async () => {});
  rerender({ ...input, currentPath: "/new" });
  expect(mocks.listen).toHaveBeenCalledTimes(1);
  const listener: DragListener = mocks.listen.mock.calls[0][0];
  await act(async () => { await listener(drop()); });
  expect(mocks.upload).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "workspace", sessionId: "session", localPath: "C:/file.txt", remotePath: "/new/file.txt", overwrite: false,
  });
  expect(input.onTransfer).toHaveBeenCalledTimes(1);
  unmount();
  expect(mocks.stop).toHaveBeenCalledTimes(1);
});

it("does not start uploads after unmount while native hit testing is pending", async () => {
  let finishScale!: (scale: number) => void;
  mocks.scale.mockReturnValueOnce(new Promise((resolve) => { finishScale = resolve; }));
  const input = options();
  const { unmount } = renderHook(() => useSftpNativeDragDrop(input));
  await act(async () => {});
  const listener: DragListener = mocks.listen.mock.calls[0][0];
  const pending = listener(drop());
  unmount();
  await act(async () => { finishScale(1); await pending; });
  expect(mocks.upload).not.toHaveBeenCalled();
  expect(input.onTransfer).not.toHaveBeenCalled();
});

it("does not start the next dropped file after an in-flight upload outlives the panel", async () => {
  let finishUpload!: () => void;
  mocks.upload.mockReturnValueOnce(new Promise<void>((resolve) => { finishUpload = resolve; }));
  const input = options();
  const { unmount } = renderHook(() => useSftpNativeDragDrop(input));
  await act(async () => {});
  const listener: DragListener = mocks.listen.mock.calls[0][0];
  let pending: ReturnType<DragListener>;
  await act(async () => { pending = listener(drop(["C:/first.txt", "C:/second.txt"])); });
  expect(mocks.upload).toHaveBeenCalledTimes(1);
  unmount();
  await act(async () => { finishUpload(); await pending; });
  expect(mocks.upload).toHaveBeenCalledTimes(1);
  expect(input.onTransfer).not.toHaveBeenCalled();
});
