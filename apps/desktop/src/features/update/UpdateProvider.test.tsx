// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { UpdateDownloadEvent, UpdateInfo, UpdateMeta } from "./updateTypes";

const mocks = vi.hoisted(() => ({ info: vi.fn(), check: vi.fn(), install: vi.fn(), restart: vi.fn(), listen: vi.fn() }));
vi.mock("./updateApi", () => ({
  getUpdateInfo: mocks.info,
  checkForUpdate: mocks.check,
  installUpdate: mocks.install,
  listenForInstalledUpdate: mocks.listen,
  restartAfterUpdate: mocks.restart,
  updaterError: () => ({ message: "failed", recovery: "download" }),
}));
vi.mock("./updateCheckPolicy", () => ({
  recordSuccessfulUpdateCheck: vi.fn(), wasUpdateCheckedRecently: () => false,
}));
import { UpdateProvider } from "./UpdateProvider";
import { useUpdate } from "./useUpdate";

const meta: UpdateMeta = {
  name: "Unfour", version: "0.9.6", distribution: "standard", channel: "stable",
  commit: null, updaterEnabled: true, installerExitsApp: false, installedVersion: null, endpoint: null,
};

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mocks.listen.mockResolvedValue(() => {});
  mocks.info.mockResolvedValue(meta);
  mocks.check.mockResolvedValue({ version: "0.9.7", currentVersion: "0.9.6", date: null, body: null });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it("installs the latest committed update once and clears pending progress timers", async () => {
  let progress!: (event: UpdateDownloadEvent) => void;
  let finish!: () => void;
  mocks.install.mockImplementation((callback: typeof progress) => {
    progress = callback;
    return new Promise<void>((resolve) => { finish = resolve; });
  });
  const { result, rerender, unmount } = renderHook(useUpdate, { wrapper: UpdateProvider });
  await act(async () => {});
  await act(() => result.current.check());
  expect(result.current.state.kind).toBe("available");
  let install!: Promise<void>;
  act(() => { install = result.current.install(); });
  rerender();
  await act(() => result.current.install());
  expect(mocks.install).toHaveBeenCalledTimes(1);
  act(() => { progress({ event: "started", contentLength: 100 }); progress({ event: "progress", chunkLength: 30 }); });
  await act(() => vi.advanceTimersByTimeAsync(120));
  expect(result.current.state).toMatchObject({ kind: "downloading", downloaded: 30, info: { version: "0.9.7" } });
  act(() => progress({ event: "downloaded" }));
  expect(result.current.state.kind).toBe("installing");
  await act(async () => { finish(); await install; });
  expect(result.current.state).toMatchObject({ kind: "installed", info: { version: "0.9.7" } });
  act(() => progress({ event: "installing" }));
  await act(() => result.current.check());
  await act(() => result.current.install());
  expect(result.current.state.kind).toBe("installed");
  expect(mocks.install).toHaveBeenCalledTimes(1);
  expect(mocks.restart).not.toHaveBeenCalled();
  await act(() => result.current.restart());
  expect(mocks.restart).toHaveBeenCalledTimes(1);
  unmount();
  expect(vi.getTimerCount()).toBe(0);
  await vi.advanceTimersByTimeAsync(5000);
  expect(mocks.check).toHaveBeenCalledTimes(1);
});

it("keeps one delayed automatic check across ordinary provider rerenders", async () => {
  const { rerender, unmount } = renderHook(useUpdate, { wrapper: UpdateProvider });
  await act(async () => {});
  rerender();
  rerender();
  await act(() => vi.advanceTimersByTimeAsync(4999));
  expect(mocks.check).not.toHaveBeenCalled();
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(mocks.check).toHaveBeenCalledTimes(1);
  expect(mocks.info).toHaveBeenCalledTimes(1);
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});

it("restores the pending restart after reloading the WebView", async () => {
  mocks.info.mockResolvedValue({ ...meta, installedVersion: "0.9.7" });
  const { result } = renderHook(useUpdate, { wrapper: UpdateProvider });
  await act(async () => {});
  expect(result.current.state).toMatchObject({ kind: "installed", info: { version: "0.9.7" } });
  await act(() => result.current.check());
  expect(mocks.check).not.toHaveBeenCalled();
});

it("receives completion from the previous WebView during a check and keeps it after a late response", async () => {
  let notify!: (info: UpdateInfo) => void;
  const unlisten = vi.fn();
  mocks.listen.mockImplementation(async (handler) => { notify = handler; return unlisten; });
  let finishCheck!: (info: UpdateInfo | null) => void;
  mocks.check.mockImplementation(() => new Promise((resolve) => { finishCheck = resolve; }));
  const { result, unmount } = renderHook(useUpdate, { wrapper: UpdateProvider });
  await act(async () => {});
  expect(mocks.listen.mock.invocationCallOrder[0]).toBeLessThan(mocks.info.mock.invocationCallOrder[0]);
  let checking!: Promise<void>;
  act(() => { checking = result.current.check(); });
  const committed = { version: "0.9.7", currentVersion: meta.version, date: null, body: "Release notes" };
  act(() => notify(committed));
  await act(async () => { finishCheck(null); await checking; });
  expect(result.current.state).toEqual({ kind: "installed", info: committed });
  await act(() => result.current.restart());
  expect(mocks.restart).toHaveBeenCalledTimes(1);
  unmount();
  expect(unlisten).toHaveBeenCalledTimes(1);
});
