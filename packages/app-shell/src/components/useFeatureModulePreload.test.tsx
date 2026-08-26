// @vitest-environment jsdom
import { QueryClient } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useFeatureModulePreload } from "./useFeatureModulePreload";

let idleCallbacks: Array<() => void> = [];

beforeEach(() => {
  idleCallbacks = [];
  Object.defineProperty(window, "requestIdleCallback", {
    configurable: true,
    value: vi.fn((callback: () => void) => {
      idleCallbacks.push(callback);
      return idleCallbacks.length;
    }),
  });
  Object.defineProperty(window, "cancelIdleCallback", {
    configurable: true,
    value: vi.fn(),
  });
});

afterEach(() => {
  cleanup();
  Reflect.deleteProperty(window, "requestIdleCallback");
  Reflect.deleteProperty(window, "cancelIdleCallback");
});

describe("useFeatureModulePreload", () => {
  it("warms inactive modules one at a time while the browser is idle", async () => {
    const preload = vi.fn().mockResolvedValue(undefined);
    const queryClient = new QueryClient();
    renderHook(() =>
      useFeatureModulePreload("api", {
        preload,
        queryClient,
        workspaceId: "workspace-one",
      }),
    );

    expect(idleCallbacks).toHaveLength(1);
    await act(async () => idleCallbacks.shift()?.());
    expect(preload).toHaveBeenLastCalledWith("database", {
      queryClient,
      workspaceId: "workspace-one",
    });

    await waitFor(() => expect(idleCallbacks).toHaveLength(1));
    await act(async () => idleCallbacks.shift()?.());
    expect(preload).toHaveBeenLastCalledWith("ssh", {
      queryClient,
      workspaceId: "workspace-one",
    });
    expect(preload).toHaveBeenCalledTimes(2);
  });
});
