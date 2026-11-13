import { afterEach, expect, it, vi } from "vitest";
import { call } from "./invoke";
import { databaseActionContext, listDatabaseConnections, saveDatabaseConnection, executeDatabaseQuery, testDatabaseConnectionInput } from "./database";
import { saveRedisConnection, inspectRedisKey } from "./redis";
import { saveSshConnection, connectSshSession, testSshConnection, runSshTask } from "./ssh";
import { cancelTemporarySsh, listTemporaryRouteHandles } from "./temporary-ssh";
import type { DatabaseConnection } from "../types";

vi.mock("./invoke", () => ({ call: vi.fn(), isTauriRuntime: () => true }));
const handle = "handle-test-12345678";
const scope = { workspaceId: "workspace-a", connectionId: "target" };
const db = { driver: "postgres" as const, name: "target", workspaceId: scope.workspaceId, temporaryHandle: handle };
const ssh = { name: "target", username: "user", host: "fixture", authKind: "none" as const, workspaceId: scope.workspaceId, temporaryHandle: handle };
afterEach(() => vi.resetAllMocks());

it("only lists connected sources owned by the requested workspace", async () => {
  vi.mocked(call).mockResolvedValueOnce([
    { handle, status: "connected", workspaceId: scope.workspaceId },
    { handle: "foreign", status: "connected", workspaceId: "other" },
    { handle: "pending", status: "pending", workspaceId: null },
    { handle: "closed", status: "disconnected", workspaceId: scope.workspaceId },
  ]);
  expect(await listTemporaryRouteHandles(scope.workspaceId)).toEqual([handle]);
});

it("binds explicit SQL and SSH actions without including the handle in a connection save", async () => {
  vi.mocked(call).mockResolvedValue({ id: scope.connectionId });
  await saveDatabaseConnection(db);
  await saveSshConnection(ssh);
  expect(JSON.stringify(vi.mocked(call).mock.calls)).not.toContain(handle);
  await testDatabaseConnectionInput(db, null);
  await executeDatabaseQuery({ ...scope, sql: "SELECT 1" });
  await testSshConnection(ssh);
  await connectSshSession(scope);
  for (const [, args] of vi.mocked(call).mock.calls.slice(2)) {
    expect(args?.temporaryHandle).toBe(handle);
    expect(JSON.stringify(args?.input)).not.toContain(handle);
  }
  await connectSshSession({ ...scope, workspaceId: "workspace-b" });
  expect(vi.mocked(call).mock.lastCall?.[1]).not.toHaveProperty("temporaryHandle");
  await connectSshSession(scope, "original-session");
  expect(vi.mocked(call).mock.lastCall?.[1]).toEqual({ input: scope, sourceSessionId: "original-session" });
  await runSshTask({ ...scope, taskId: "task", inputs: {}, secretInputNames: [] });
  expect(vi.mocked(call).mock.lastCall?.[1]).toMatchObject({ temporaryHandle: handle });
});

it("pins the SQL target displayed for a query, across another window's saved-route edit", async () => {
  const before = { id: "db-cas", workspaceId: "ws-cas", revision: 8, sshConnectionId: "a", sshRevision: 1 } as DatabaseConnection;
  vi.mocked(call).mockResolvedValueOnce([before]);
  await listDatabaseConnections(before.workspaceId);
  const context = databaseActionContext(before.workspaceId, before.id, before);
  const input = { workspaceId: before.workspaceId, connectionId: before.id, sql: "SELECT 1", ...context };
  await executeDatabaseQuery(input);
  expect(vi.mocked(call).mock.lastCall?.[1]).toEqual({ input: { workspaceId: before.workspaceId, connectionId: before.id, sql: "SELECT 1" },
    expectedConnection: { revision: 8, sshConnectionId: "a", sshRevision: 1 } });
  vi.mocked(call).mockResolvedValueOnce([{ ...before, sshConnectionId: "b" }]);
  await listDatabaseConnections(before.workspaceId);
  expect(() => executeDatabaseQuery(input)).toThrow();
});

it("a late Redis save cannot restore a password after the source disappears in another window", async () => {
  const workspaceId = "ws-redis-remote";
  const secret = { handle, password: "runtime-secret" };
  const input = { workspaceId, name: "cache", host: "fixture", port: 6379, dbIndex: 0, username: null, tls: true };
  vi.mocked(call).mockResolvedValueOnce([{ handle, status: "connected", workspaceId }]);
  await listTemporaryRouteHandles(workspaceId);
  let finish!: (value: unknown) => void;
  vi.mocked(call).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  const saving = saveRedisConnection(input, null, secret);
  vi.mocked(call).mockResolvedValueOnce([]);
  await listTemporaryRouteHandles(workspaceId);
  finish({ id: "cache", revision: 1 });
  await saving;
  await inspectRedisKey(workspaceId, "cache", "aw==", undefined, 1);
  expect(vi.mocked(call).mock.lastCall?.[1]).toMatchObject({ temporaryHandle: handle });
  expect(vi.mocked(call).mock.lastCall?.[1]).not.toHaveProperty("temporaryRedisPassword");
});

it("drops the per-session Redis password on close and retains the expired route until explicitly changed", async () => {
  vi.mocked(call).mockResolvedValue({ id: scope.connectionId, revision: 1 });
  const input = { workspaceId: scope.workspaceId, name: "cache", host: "fixture", port: 6379, dbIndex: 0, username: null, tls: true };
  await saveRedisConnection(input, null, { handle, password: "once" });
  expect(vi.mocked(call).mock.lastCall).toEqual(["redis_connection_save", { input, secret: null }]);
  await inspectRedisKey(scope.workspaceId, scope.connectionId, "aw==", undefined, 1);
  expect(vi.mocked(call).mock.lastCall?.[1]).toMatchObject({ temporaryHandle: handle, temporaryRedisPassword: "once" });
  await inspectRedisKey(scope.workspaceId, scope.connectionId, "aw==", undefined, 2);
  expect(vi.mocked(call).mock.lastCall?.[1]).not.toHaveProperty("temporaryRedisPassword");
  await cancelTemporarySsh(scope.workspaceId, handle);
  await inspectRedisKey(scope.workspaceId, scope.connectionId, "aw==", undefined, 1);
  expect(vi.mocked(call).mock.lastCall?.[1]).toMatchObject({ temporaryHandle: handle });
  expect(vi.mocked(call).mock.lastCall?.[1]).not.toHaveProperty("temporaryRedisPassword");
});
