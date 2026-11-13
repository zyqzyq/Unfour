// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { saveDatabaseConnection } from "./database";
import { saveSshConnection, saveSshTask } from "./ssh";
import type { DatabaseConnection, SshConnection, SshTaskDetail } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks(); });

it("forwards editor snapshots as separate save preconditions", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const database = { id: "db", workspaceId: "ws", revision: 2 } as DatabaseConnection;
  const databaseInput = { id: "db", workspaceId: "ws", name: "Draft", driver: "sqlite" as const };
  vi.mocked(invoke).mockResolvedValueOnce({ ...database, ...databaseInput });
  await saveDatabaseConnection({ ...databaseInput, expectedSnapshot: database });
  expect(invoke).toHaveBeenLastCalledWith("database_connection_save", { input: databaseInput, secret: null, expectedSnapshot: database });
  const ssh = { id: "ssh", workspaceId: "ws", revision: 2 } as SshConnection;
  const sshInput = { id: "ssh", workspaceId: "ws", name: "Draft", host: "fixture", username: "dev", authKind: "none" as const };
  vi.mocked(invoke).mockResolvedValueOnce({ ...ssh, ...sshInput });
  await saveSshConnection({ ...sshInput, expectedSnapshot: ssh });
  expect(invoke).toHaveBeenLastCalledWith("ssh_connection_save", { input: sshInput, expectedSnapshot: ssh });
  const task = { task: { id: "task" }, steps: [], localBinding: null, detectedInputs: [] } as unknown as SshTaskDetail;
  const taskInput = { id: "task", workspaceId: "ws", name: "Draft", description: "", steps: [] };
  await saveSshTask({ ...taskInput, expectedSnapshot: task });
  expect(invoke).toHaveBeenLastCalledWith("ssh_task_save", { input: taskInput, expectedSnapshot: task });
});
