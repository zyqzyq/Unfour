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
    expect(closeMutation.mutate).toHaveBeenCalledWith("s2");
    expect(connectMutation.mutate).toHaveBeenCalledWith("c2");
  });
});
