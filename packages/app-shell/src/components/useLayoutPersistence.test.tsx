// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  save: vi.fn().mockResolvedValue(undefined), error: vi.fn(),
  layout: { layoutWorkspaceId: "ws", activeTabId: "api", tabs: [], snapshotLayout: vi.fn(() => ({ sidebarWidth: 280 })) },
}));
vi.mock("@unfour/workspace-core", () => ({ useWorkspaceStore: () => mocks.layout }));
vi.mock("@unfour/command-client", () => ({ updateWorkspaceLayout: mocks.save }));
vi.mock("@unfour/ui", () => ({ useFeedbackErrorHandler: () => mocks.error }));
import { useLayoutPersistence } from "./useLayoutPersistence";

afterEach(() => { cleanup(); vi.useRealTimers(); });

it("preserves the debounce across mutation rerenders and cancels unsaved timers on unmount", async () => {
  vi.useFakeTimers();
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  }
  const { rerender, unmount } = renderHook(() => useLayoutPersistence("ws"), { wrapper: Wrapper });
  await act(() => vi.advanceTimersByTimeAsync(200));
  rerender();
  await act(() => vi.advanceTimersByTimeAsync(150));
  expect(mocks.save).toHaveBeenCalledExactlyOnceWith("ws", { sidebarWidth: 280 });
  await act(() => vi.advanceTimersByTimeAsync(1000));
  expect(mocks.save).toHaveBeenCalledTimes(1);
  mocks.layout = { ...mocks.layout, activeTabId: "database" };
  rerender();
  unmount();
  await vi.advanceTimersByTimeAsync(1000);
  expect(mocks.save).toHaveBeenCalledTimes(1);
  client.clear();
});
