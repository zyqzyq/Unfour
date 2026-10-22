// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { subscribeCloudSyncWorkspaceChanges } from "./cloud-sync";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); vi.resetAllMocks(); });

it("forwards committed workspace IDs and returns the native cleanup function", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const stop = vi.fn();
  const onChange = vi.fn();
  vi.mocked(listen).mockResolvedValue(stop);
  expect(await subscribeCloudSyncWorkspaceChanges(onChange)).toBe(stop);
  expect(listen).toHaveBeenCalledWith("cloud-sync://workspace-changed", expect.any(Function));
  const callback = vi.mocked(listen).mock.calls[0][1];
  callback({ event: "cloud-sync://workspace-changed", id: 1, payload: { workspaceId: "ws" } });
  expect(onChange).toHaveBeenCalledWith("ws");
});

it("keeps browser previews independent of Tauri events", async () => {
  expect(await subscribeCloudSyncWorkspaceChanges(vi.fn())).toBeNull();
  expect(listen).not.toHaveBeenCalled();
});
