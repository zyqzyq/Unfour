// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
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

  it("keeps a new hydration marker when an obsolete request rejects", async () => {
    let rejectOld!: (error: Error) => void;
    historyMock.mockImplementationOnce(() => new Promise((_, reject) => { rejectOld = reject; }));
    const hydrate = vi.fn();
    const { rerender } = renderHook(({ active }) => useActiveSessionHistory({
      active, hydrate, sessionId: "session-one", workspaceId: "workspace-one",
    }), { initialProps: { active: true } });

    rerender({ active: false });
    rerender({ active: true });
    await waitFor(() => expect(hydrate).toHaveBeenCalledTimes(1));
    await act(async () => { rejectOld(new Error("obsolete failure")); });
    rerender({ active: false });
    rerender({ active: true });
    expect(historyMock).toHaveBeenCalledTimes(2);
  });

  it("retries a current failure when the session becomes active again", async () => {
    historyMock.mockRejectedValueOnce(new Error("temporary failure"));
    const hydrate = vi.fn();
    const { rerender } = renderHook(({ active }) => useActiveSessionHistory({
      active, hydrate, sessionId: "session-one", workspaceId: "workspace-one",
    }), { initialProps: { active: true } });
    await act(async () => {});
    rerender({ active: false });
    rerender({ active: true });
    await waitFor(() => expect(hydrate).toHaveBeenCalledTimes(1));
    expect(historyMock).toHaveBeenCalledTimes(2);
  });

  it("ignores completed reads after a workspace switch and unmount", async () => {
    const resolveReads: Array<(events: []) => void> = [];
    historyMock.mockImplementation(() => new Promise((resolve) => { resolveReads.push(resolve); }));
    const hydrate = vi.fn();
    const { rerender, unmount } = renderHook(({ workspaceId }) => useActiveSessionHistory({
      active: true, hydrate, sessionId: "same-session", workspaceId,
    }), { initialProps: { workspaceId: "workspace-one" } });
    rerender({ workspaceId: "workspace-two" });
    await act(async () => { resolveReads[0]([]); });
    expect(hydrate).not.toHaveBeenCalled();
    unmount();
    await act(async () => { resolveReads[1]([]); });
    expect(hydrate).not.toHaveBeenCalled();
  });
});
