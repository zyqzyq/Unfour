// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import {
  resetDatabaseConnectionStore,
  useDatabaseConnectionStore,
} from "./database-connection-state";

describe("useDatabaseConnectionStore", () => {
  beforeEach(() => {
    resetDatabaseConnectionStore();
  });

  it("keeps connection state isolated per workspace", () => {
    const store = useDatabaseConnectionStore.getState();
    store.setConnectionState("ws-a", "conn-1", { status: "connected" });
    store.setConnectionState("ws-b", "conn-1", { status: "connecting" });

    const a = useDatabaseConnectionStore.getState().byWorkspace["ws-a"]["conn-1"].status;
    const b = useDatabaseConnectionStore.getState().byWorkspace["ws-b"]["conn-1"].status;
    expect(a).toBe("connected");
    expect(b).toBe("connecting");

    // Removing from ws-b must not touch ws-a.
    useDatabaseConnectionStore.getState().removeConnection("ws-b", "conn-1");
    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-b"]).toEqual({});
    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-a"]["conn-1"].status).toBe(
      "connected",
    );

    // Pruning ws-a must not touch ws-b (re-add ws-b first to verify isolation).
    store.setConnectionState("ws-b", "conn-2", { status: "connected" });
    useDatabaseConnectionStore.getState().pruneConnections("ws-a", new Set());
    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-a"]).toEqual({});
    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-b"]["conn-2"].status).toBe(
      "connected",
    );
  });

  it("resets a single workspace via resetDatabaseConnectionStore", () => {
    const store = useDatabaseConnectionStore.getState();
    store.setConnectionState("ws-a", "conn-1", { status: "connected" });
    store.setConnectionState("ws-b", "conn-1", { status: "connected" });

    resetDatabaseConnectionStore("ws-a");

    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-a"]).toBeUndefined();
    expect(useDatabaseConnectionStore.getState().byWorkspace["ws-b"]["conn-1"].status).toBe(
      "connected",
    );
  });
});
