// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { UpdateDownloadEvent, UpdateMeta } from "./updateTypes";

const mocks = vi.hoisted(() => ({ info: vi.fn(), check: vi.fn(), install: vi.fn() }));
vi.mock("./updateApi", () => ({
  getUpdateInfo: mocks.info,
  checkForUpdate: mocks.check,
  installUpdate: mocks.install,
  updaterError: () => ({ message: "failed", recovery: "download" }),
}));
vi.mock("./updateCheckPolicy", () => ({
  recordSuccessfulUpdateCheck: vi.fn(), wasUpdateCheckedRecently: () => false,
}));
import { UpdateProvider } from "./UpdateProvider";
import { useUpdate } from "./useUpdate";

const meta: UpdateMeta = {
  name: "Unfour", version: "0.9.0", distribution: "standard", channel: "stable",
  commit: null, updaterEnabled: true, endpoint: null,
};

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mocks.info.mockResolvedValue(meta);
  mocks.check.mockResolvedValue({ version: "0.9.1", currentVersion: "0.9.0", date: null, body: null });
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
  expect(result.current.state).toMatchObject({ kind: "downloading", downloaded: 30, info: { version: "0.9.1" } });
  act(() => progress({ event: "downloaded" }));
  expect(result.current.state.kind).toBe("installing");
  await act(async () => { finish(); await install; });
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
