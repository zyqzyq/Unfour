// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiHistoryDetail, ApiHistoryItem } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  getApiHistoryDetail: vi.fn(),
  listApiHistory: vi.fn(),
}));

import { getApiHistoryDetail, listApiHistory } from "@unfour/command-client";
import { useApiHistory } from "./useApiHistory";

const listMock = vi.mocked(listApiHistory);
const detailMock = vi.mocked(getApiHistoryDetail);

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("useApiHistory", () => {
  it("loads the history list for the workspace", async () => {
    listMock.mockResolvedValue([{ id: "h1" } as ApiHistoryItem]);

    const { result } = renderHook(
      () => useApiHistory({ onReplayLoaded: vi.fn(), workspaceId: "ws-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() =>
      expect(result.current.historyQuery.data).toHaveLength(1),
    );
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("does not load history while the workspace id is empty", () => {
    renderHook(
      () => useApiHistory({ onReplayLoaded: vi.fn(), workspaceId: "" }),
      { wrapper: createWrapper() },
    );
    expect(listMock).not.toHaveBeenCalled();
  });

  it("replays a history entry and forwards the detail to the callback", async () => {
    listMock.mockResolvedValue([]);
    const detail = { id: "h1", method: "GET" } as unknown as ApiHistoryDetail;
    detailMock.mockResolvedValue(detail);
    const onReplayLoaded = vi.fn();

    const { result } = renderHook(
      () => useApiHistory({ onReplayLoaded, workspaceId: "ws-1" }),
      { wrapper: createWrapper() },
    );

    result.current.replayHistoryMutation.mutate("h1");

    await waitFor(() => expect(detailMock).toHaveBeenCalledWith("ws-1", "h1"));
    await waitFor(() => expect(onReplayLoaded).toHaveBeenCalled());
    expect(onReplayLoaded.mock.calls[0][0]).toBe(detail);
  });
});
