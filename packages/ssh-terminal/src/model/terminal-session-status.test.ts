import { describe, expect, it } from "vitest";
import type { SshSessionSummary } from "@unfour/command-client";
import {
  shouldRenderTerminalPane,
  terminalSessionStatus,
  terminalSessionStatusLabel,
} from "./terminal-session-status";

function session(
  status: SshSessionSummary["status"],
  overrides: Partial<SshSessionSummary> = {},
): SshSessionSummary {
  return {
    authKind: "password",
    cols: 120,
    connectionId: "connection-1",
    createdAt: "2026-01-01T00:00:00Z",
    host: "example.test",
    reconnectAttempt: 0,
    rows: 32,
    sessionId: `session-${status}`,
    status,
    updatedAt: "2026-01-01T00:00:00Z",
    username: "dev",
    workspaceId: "workspace-1",
    ...overrides,
  };
}

describe("terminalSessionStatus", () => {
  it("maps failed sessions to error and missing sessions to disconnected", () => {
    expect(terminalSessionStatus(null)).toBe("disconnected");
    expect(terminalSessionStatus(session("failed"))).toBe("error");
  });

  it("maps degraded and reconnecting to a reconnecting state", () => {
    expect(terminalSessionStatus(session("degraded"))).toBe("reconnecting");
    expect(terminalSessionStatus(session("reconnecting"))).toBe("reconnecting");
  });

  it("passes through connected and disconnected states", () => {
    expect(terminalSessionStatus(session("connected"))).toBe("connected");
    expect(terminalSessionStatus(session("disconnected"))).toBe("disconnected");
  });
});

describe("terminalSessionStatusLabel", () => {
  it("returns disconnected when there is no session", () => {
    expect(terminalSessionStatusLabel(null)).toBe("disconnected");
  });

  it("shows the reconnect attempt count while reconnecting", () => {
    expect(
      terminalSessionStatusLabel(session("reconnecting", { reconnectAttempt: 2 })),
    ).toBe("reconnecting 2/3");
  });

  it("describes degraded connections and passes other statuses through", () => {
    expect(terminalSessionStatusLabel(session("degraded"))).toBe(
      "connection degraded",
    );
    expect(terminalSessionStatusLabel(session("connected"))).toBe("connected");
  });
});

describe("terminal session status helpers", () => {
  it("renders live and reconnecting sessions in the terminal pane", () => {
    expect(shouldRenderTerminalPane(session("connected"))).toBe(true);
    expect(shouldRenderTerminalPane(session("degraded"))).toBe(true);
    expect(shouldRenderTerminalPane(session("reconnecting"))).toBe(true);
  });

  it("does not render empty failed or disconnected sessions as editable panes", () => {
    expect(shouldRenderTerminalPane(session("failed"))).toBe(false);
    expect(shouldRenderTerminalPane(session("disconnected"))).toBe(false);
    expect(shouldRenderTerminalPane(null)).toBe(false);
  });

  it("keeps failed or disconnected sessions with output available for log review", () => {
    expect(shouldRenderTerminalPane(session("failed"), 1)).toBe(true);
    expect(shouldRenderTerminalPane(session("disconnected"), 1)).toBe(true);
  });
});
