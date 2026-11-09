// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useLayoutPersistence } from "./useLayoutPersistence";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn().mockResolvedValue(undefined),
  error: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@unfour/ui", () => ({ useFeedbackErrorHandler: () => mocks.error }));

afterEach(() => {
  cleanup();
  useWorkspaceStore.setState(useWorkspaceStore.getInitialState(), true);
  delete window.__TAURI_INTERNALS__;
  vi.useRealTimers();
});

it("sends integer dimensions through command-client after fractional panel resizes", async () => {
  vi.useFakeTimers();
  window.__TAURI_INTERNALS__ = {};
  const store = useWorkspaceStore.getState();
  store.hydrateLayout(store.snapshotLayout("ws-resized"));
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  }
  const { unmount } = renderHook(() => useLayoutPersistence("ws-resized"), { wrapper: Wrapper });

  act(() => {
    store.setModuleSidebarWidth("api", 350.75);
    store.setModuleSidebarWidth("ssh", 260.25);
    store.setModuleSidebarWidth("database", 400.5);
    store.setModuleSidebarWidth("flow", 290.5);
    store.setBottomPanelHeight(240.75);
    store.setRightInspectorWidth(310.25);
  });
  await act(() => vi.advanceTimersByTimeAsync(350));

  expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("workspace_layout_update", {
    workspaceId: "ws-resized",
    layout: expect.objectContaining({
      workspaceId: "ws-resized",
      sidebarWidths: { api: 351, ssh: 260, database: 401, flow: 291 },
      bottomPanelHeight: 241,
      rightInspectorWidth: 310,
    }),
  });
  expect(mocks.error).not.toHaveBeenCalled();
  unmount();
  client.clear();
});
