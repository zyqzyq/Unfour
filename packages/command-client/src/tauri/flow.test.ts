// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { runFlow } from "./flow";
import type { FlowRunInput } from "../types/flow";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks(); });

it("passes the confirmed revision as a required Tauri argument", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const input: FlowRunInput = {
    workspaceId: "ws", flowId: "flow", environmentId: null,
    inputs: {}, secretInputNames: [], initiator: "human", confirmEffects: true,
  };
  await runFlow(input, 3);
  expect(invoke).toHaveBeenCalledWith("flow_run", { input, expectedRevision: 3 });
});

it("keeps the Tauri adapter revision argument required and forwards it to the pinned bus entry", () => {
  const source = readFileSync("crates/unfour-app/src/commands/flow.rs", "utf8");
  const command = source.slice(source.indexOf("pub async fn flow_run("));
  expect(command).toMatch(/expected_revision: i64/);
  expect(command).toMatch(/run_flow_at_revision\(input, Some\(expected_revision\)\)/);
  expect(command).not.toMatch(/\.run_flow\(/);
});
