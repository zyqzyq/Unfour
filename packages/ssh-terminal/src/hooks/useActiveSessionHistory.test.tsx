// @vitest-environment jsdom
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getSshSessionHistory } from "@unfour/command-client";
import { useActiveSessionHistory } from "./useActiveSessionHistory";

vi.mock("@unfour/command-client", () => ({
  getSshSessionHistory: vi.fn(),
}));

const historyMock = vi.mocked(getSshSessionHistory);

beforeEach(() => {
  historyMock.mockReset().mockResolvedValue([]);
});

afterEach(cleanup);

describe("useActiveSessionHistory", () => {
  it("hydrates only the active session and loads another session on selection", async () => {
    const hydrate = vi.fn();
    const { rerender } = renderHook(
      ({ sessionId }: { sessionId?: string }) =>
        useActiveSessionHistory({
          active: true,
          hydrate,
          sessionId,
          workspaceId: "workspace-one",
        }),
      { initialProps: { sessionId: "session-one" } },
    );

    await waitFor(() => expect(hydrate).toHaveBeenCalledWith("session-one", []));
    rerender({ sessionId: "session-two" });
    await waitFor(() => expect(hydrate).toHaveBeenCalledWith("session-two", []));
    expect(historyMock).toHaveBeenCalledTimes(2);
  });

  it("does not load history while the SSH surface is inactive", () => {
    renderHook(() =>
      useActiveSessionHistory({
        active: false,
        hydrate: vi.fn(),
        sessionId: "session-one",
        workspaceId: "workspace-one",
      }),
    );

    expect(historyMock).not.toHaveBeenCalled();
  });
});
