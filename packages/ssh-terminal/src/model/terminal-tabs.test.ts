import type { SshConnection, SshSessionSummary } from "@unfour/command-client";
import { describe, expect, it } from "vitest";
import {
  buildTerminalSessionTabs,
  shouldCloseTerminalSessionInBackend,
  shouldShowTerminalSessionTab,
  terminalBatchCloseTabs,
} from "./terminal-tabs";

function connection(overrides: Partial<SshConnection> & { id: string }): SshConnection {
  return {
    workspaceId: "ws-1",
    name: "Host",
    host: "10.0.0.1",
    port: 22,
    username: "ops",
    authKind: "password",
    keyPath: null,
    credentialRef: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "synced",
    remoteId: null,
    ...overrides,
  };
}

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

describe("buildTerminalSessionTabs", () => {
  it("uses the connection name as the tab title when matched", () => {
    const tabs = buildTerminalSessionTabs({
      connections: [connection({ id: "c1", name: "Prod DB" })],
      sessions: [session({ sessionId: "s1", connectionId: "c1" })],
    });

    expect(tabs).toHaveLength(1);
    expect(tabs[0].title).toBe("Prod DB");
    expect(tabs[0].connection?.id).toBe("c1");
  });

  it("falls back to user@host when no connection matches", () => {
    const tabs = buildTerminalSessionTabs({
      connections: [],
      sessions: [
        session({
          sessionId: "s1",
          connectionId: "missing",
          username: "deploy",
          host: "box",
        }),
      ],
    });

    expect(tabs[0].connection).toBeNull();
    expect(tabs[0].title).toBe("deploy@box");
  });

  it("disambiguates duplicate titles with an incrementing suffix", () => {
    const conns = [connection({ id: "c1", name: "Prod DB" })];
    const tabs = buildTerminalSessionTabs({
      connections: conns,
      sessions: [
        session({ sessionId: "s1", connectionId: "c1" }),
        session({ sessionId: "s2", connectionId: "c1" }),
        session({ sessionId: "s3", connectionId: "c1" }),
      ],
    });

    expect(tabs.map((tab) => tab.title)).toEqual([
      "Prod DB",
      "Prod DB 2",
      "Prod DB 3",
    ]);
  });
});

describe("shouldShowTerminalSessionTab", () => {
  it("hides disconnected restored history by default", () => {
    expect(
      shouldShowTerminalSessionTab({
        activeSessionId: null,
        dismissedSessionIds: [],
        session: session({
          sessionId: "s1",
          connectionId: "c1",
          status: "disconnected",
        }),
      }),
    ).toBe(false);
  });

  it("keeps the current disconnected session visible for review", () => {
    expect(
      shouldShowTerminalSessionTab({
        activeSessionId: "s1",
        dismissedSessionIds: [],
        session: session({
          sessionId: "s1",
          connectionId: "c1",
          status: "disconnected",
        }),
      }),
    ).toBe(true);
  });

  it("keeps a failed connection tab visible after another tab becomes active", () => {
    expect(
      shouldShowTerminalSessionTab({
        activeSessionId: "s2",
        dismissedSessionIds: [],
        session: session({
          sessionId: "s1",
          connectionId: "c1",
          status: "failed",
        }),
      }),
    ).toBe(true);
  });

  it("hides explicitly dismissed sessions", () => {
    expect(
      shouldShowTerminalSessionTab({
        activeSessionId: "s1",
        dismissedSessionIds: ["s1"],
        session: session({ sessionId: "s1", connectionId: "c1" }),
      }),
    ).toBe(false);
  });
});

describe("shouldCloseTerminalSessionInBackend", () => {
  it("skips the backend close command for frontend-only failed sessions", () => {
    const failedSession = session({
      sessionId: "frontend-failed-1",
      connectionId: "c1",
      status: "failed",
    });

    expect(
      shouldCloseTerminalSessionInBackend({
        frontendFailedSessions: { [failedSession.sessionId]: failedSession },
        sessionId: failedSession.sessionId,
      }),
    ).toBe(false);
  });

  it("closes backend-managed sessions through the command bus", () => {
    expect(
      shouldCloseTerminalSessionInBackend({
        frontendFailedSessions: {},
        sessionId: "backend-session-1",
      }),
    ).toBe(true);
  });
});

describe("terminalBatchCloseTabs", () => {
  const tabs = buildTerminalSessionTabs({
    connections: [connection({ id: "c1", name: "Prod DB" })],
    sessions: [
      session({ sessionId: "s1", connectionId: "c1" }),
      session({ sessionId: "s2", connectionId: "c1" }),
      session({ sessionId: "s3", connectionId: "c1" }),
    ],
  });

  it("selects every session for close all", () => {
    expect(terminalBatchCloseTabs(tabs, "all").map((tab) => tab.session.sessionId)).toEqual([
      "s1",
      "s2",
      "s3",
    ]);
  });

  it("selects other sessions, left sessions, and right sessions", () => {
    expect(terminalBatchCloseTabs(tabs, "others", "s2").map((tab) => tab.session.sessionId)).toEqual([
      "s1",
      "s3",
    ]);
    expect(terminalBatchCloseTabs(tabs, "left", "s2").map((tab) => tab.session.sessionId)).toEqual([
      "s1",
    ]);
    expect(terminalBatchCloseTabs(tabs, "right", "s1").map((tab) => tab.session.sessionId)).toEqual([
      "s2",
      "s3",
    ]);
  });

  it("returns no sessions when close left is requested on the first tab", () => {
    expect(terminalBatchCloseTabs(tabs, "left", "s1")).toEqual([]);
  });
});
