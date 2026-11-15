// @vitest-environment jsdom
import type { SshSessionSummary } from "@unfour/command-client";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TerminalSessionTabState } from "../model/types";
import { useTerminalSessionActions } from "./useTerminalSessionActions";

function session(
  overrides: Partial<SshSessionSummary> & { sessionId: string; connectionId: string },
): SshSessionSummary {
  return {
    workspaceId: "ws-1",
    status: "connected",
    reconnectAttempt: 0,
    authKind: "password",
    host: "10.0.0.1",
    username: "ops",
    cols: 80,
    rows: 24,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function tab(item: SshSessionSummary, title: string): TerminalSessionTabState {
  return { session: item, title };
}

function setup(overrides: Partial<Parameters<typeof useTerminalSessionActions>[0]> = {}) {
  const sessions = [
    session({ sessionId: "s1", connectionId: "c1", host: "one.example" }),
    session({ sessionId: "s2", connectionId: "c2", host: "two.example" }),
    session({ sessionId: "s3", connectionId: "c3", host: "three.example" }),
  ];
  const closeMutation = { mutate: vi.fn() };
  const connectMutation = { mutate: vi.fn(), reset: vi.fn() };
  const dismissSession = vi.fn();
  const removeSftpSession = vi.fn();
  const hook = renderHook(() =>
    useTerminalSessionActions({
      closeMutation,
      connectMutation,
      dismissSession,
      frontendFailedSessions: {},
      removeSftpSession,
      sessions,
      sessionTabs: [
        tab(sessions[0], "one"),
        tab(sessions[1], "two"),
        tab(sessions[2], "three"),
      ],
      ...overrides,
    }),
  );
  return {
    closeMutation,
    connectMutation,
    dismissSession,
    hook,
    removeSftpSession,
    sessions,
  };
}

describe("useTerminalSessionActions batch close", () => {
  it("asks once before closing other sessions and does nothing on cancel", () => {
    const { closeMutation, dismissSession, hook, removeSftpSession } = setup();

    act(() => {
      hook.result.current.closeOtherSessions("s2");
    });

    expect(hook.result.current.batchCloseRequest).toEqual({
      labels: ["one", "three"],
      sessionIds: ["s1", "s3"],
    });
    expect(closeMutation.mutate).not.toHaveBeenCalled();

    act(() => {
      hook.result.current.setBatchCloseRequest(null);
    });

    expect(hook.result.current.batchCloseRequest).toBeNull();
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(dismissSession).not.toHaveBeenCalled();
    expect(removeSftpSession).not.toHaveBeenCalled();
  });

  it("closes only the confirmed batch targets", () => {
    const { closeMutation, dismissSession, hook } = setup();

    act(() => {
      hook.result.current.closeSessionsToLeft("s3");
    });
    act(() => {
      hook.result.current.confirmBatchClose();
    });

    expect(closeMutation.mutate.mock.calls.map((call) => call[0])).toEqual(["s1", "s2"]);
    expect(dismissSession.mock.calls.map((call) => call[0])).toEqual(["s1", "s2"]);
    expect(hook.result.current.batchCloseRequest).toBeNull();
  });

  it("closes every session after confirming close all", () => {
    const { closeMutation, hook } = setup();

    act(() => {
      hook.result.current.closeAllSessions();
    });
    expect(hook.result.current.batchCloseRequest?.sessionIds).toEqual(["s1", "s2", "s3"]);
    act(() => {
      hook.result.current.confirmBatchClose();
    });

    expect(closeMutation.mutate).toHaveBeenCalledTimes(3);
  });

  it("keeps single-session close confirmation independent of batch close", () => {
    const { closeMutation, hook } = setup();

    act(() => {
      hook.result.current.requestCloseSession("s2");
    });

    expect(hook.result.current.closeConfirmSessionId).toBe("s2");
    expect(hook.result.current.batchCloseRequest).toBeNull();
    expect(closeMutation.mutate).not.toHaveBeenCalled();

    act(() => {
      hook.result.current.confirmCloseSession();
    });

    expect(closeMutation.mutate).toHaveBeenCalledTimes(1);
    expect(closeMutation.mutate).toHaveBeenCalledWith("s2");
  });

  it("reconnects without a batch confirmation", () => {
    const { closeMutation, connectMutation, hook } = setup();

    act(() => {
      hook.result.current.reconnectSession("s2");
    });

    expect(hook.result.current.batchCloseRequest).toBeNull();
    expect(hook.result.current.closeConfirmSessionId).toBeNull();
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(connectMutation.mutate).toHaveBeenCalledWith({ connectionId: "c2", sourceSessionId: "s2" }, expect.any(Object));
    act(() => connectMutation.mutate.mock.lastCall![1].onSuccess());
    expect(closeMutation.mutate).toHaveBeenCalledWith("s2");
  });
});

describe("useTerminalSessionActions failure ownership", () => {
  it("dismisses a frontend-only failure without closing a nonexistent backend session", () => {
    const failed = session({ sessionId: "frontend-failed", connectionId: "c1", status: "failed" });
    const { closeMutation, connectMutation, dismissSession, hook, removeSftpSession } = setup({
      sessions: [failed],
      frontendFailedSessions: { [failed.sessionId]: failed },
      sessionTabs: [tab(failed, "failed")],
    });

    act(() => hook.result.current.requestCloseSession(failed.sessionId));

    expect(hook.result.current.closeConfirmSessionId).toBeNull();
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(connectMutation.reset).toHaveBeenCalledOnce();
    expect(dismissSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
    expect(removeSftpSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
  });

  it("closes a backend-managed failed session even when no confirmation is needed", () => {
    const failed = session({ sessionId: "backend-failed", connectionId: "c1", status: "failed" });
    const { closeMutation, connectMutation, dismissSession, hook, removeSftpSession } = setup({
      sessions: [failed],
      sessionTabs: [tab(failed, "failed")],
    });

    act(() => hook.result.current.requestCloseSession(failed.sessionId));

    expect(hook.result.current.closeConfirmSessionId).toBeNull();
    expect(closeMutation.mutate).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
    expect(connectMutation.reset).not.toHaveBeenCalled();
    expect(dismissSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
    expect(removeSftpSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
  });

  it("retries a frontend-only failure by connection id and dismisses it only after success", () => {
    const failed = session({ sessionId: "frontend-failed", connectionId: "c1", status: "failed" });
    const { closeMutation, connectMutation, dismissSession, hook, removeSftpSession } = setup({
      sessions: [failed],
      frontendFailedSessions: { [failed.sessionId]: failed },
      sessionTabs: [tab(failed, "failed")],
    });

    act(() => hook.result.current.reconnectSession(failed.sessionId));

    expect(connectMutation.mutate).toHaveBeenCalledExactlyOnceWith("c1", expect.any(Object));
    expect(dismissSession).not.toHaveBeenCalled();
    expect(removeSftpSession).not.toHaveBeenCalled();
    act(() => connectMutation.mutate.mock.lastCall![1].onSuccess());
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(dismissSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
    expect(removeSftpSession).toHaveBeenCalledExactlyOnceWith(failed.sessionId);
  });

  it("retains the source session and SFTP through an unsuccessful reconnect until a retry succeeds", () => {
    const { closeMutation, connectMutation, dismissSession, hook, removeSftpSession } = setup();

    act(() => hook.result.current.reconnectSession("s2"));
    // An unsuccessful mutation does not deliver its per-call onSuccess callback.
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(dismissSession).not.toHaveBeenCalled();
    expect(removeSftpSession).not.toHaveBeenCalled();

    act(() => hook.result.current.reconnectSession("s2"));
    expect(connectMutation.mutate).toHaveBeenCalledTimes(2);
    expect(connectMutation.mutate).toHaveBeenLastCalledWith(
      { connectionId: "c2", sourceSessionId: "s2" }, expect.any(Object),
    );
    expect(dismissSession).not.toHaveBeenCalled();
    expect(removeSftpSession).not.toHaveBeenCalled();
    act(() => connectMutation.mutate.mock.lastCall![1].onSuccess());

    expect(closeMutation.mutate).toHaveBeenCalledExactlyOnceWith("s2");
    expect(dismissSession).toHaveBeenCalledExactlyOnceWith("s2");
    expect(removeSftpSession).toHaveBeenCalledExactlyOnceWith("s2");
  });

  it("closes exactly the confirmed right-hand tabs with mixed frontend and backend ownership", () => {
    const kept = session({ sessionId: "kept", connectionId: "c1" });
    const connected = session({ sessionId: "connected", connectionId: "c2" });
    const frontend = session({ sessionId: "frontend", connectionId: "c3", status: "failed" });
    const backend = session({ sessionId: "backend", connectionId: "c4", status: "failed" });
    const historical = session({ sessionId: "historical", connectionId: "c5", status: "disconnected" });
    const { closeMutation, connectMutation, dismissSession, hook, removeSftpSession } = setup({
      sessions: [kept, connected, frontend, backend, historical],
      frontendFailedSessions: { [frontend.sessionId]: frontend },
      sessionTabs: [tab(kept, "one"), tab(connected, "two"), tab(frontend, "three"), tab(backend, "four")],
    });

    act(() => hook.result.current.closeSessionsToRight(kept.sessionId));
    expect(hook.result.current.batchCloseRequest).toEqual({
      labels: ["two", "three", "four"],
      sessionIds: ["connected", "frontend", "backend"],
    });
    expect(closeMutation.mutate).not.toHaveBeenCalled();
    expect(removeSftpSession).not.toHaveBeenCalled();

    act(() => hook.result.current.confirmBatchClose());

    expect(closeMutation.mutate.mock.calls).toEqual([["connected"], ["backend"]]);
    expect(connectMutation.reset).toHaveBeenCalledOnce();
    expect(dismissSession.mock.calls).toEqual([["connected"], ["frontend"], ["backend"]]);
    expect(removeSftpSession.mock.calls).toEqual([["connected"], ["frontend"], ["backend"]]);
    expect(hook.result.current.batchCloseRequest).toBeNull();
  });
});
