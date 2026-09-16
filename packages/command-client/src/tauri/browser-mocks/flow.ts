import type { FlowDefinition } from "../../types/flow";
import { UNHANDLED, type MockResult } from "./types";

// Browser preview only supports authoring. Execution always requires Rust.
const definitions = new Map<string, FlowDefinition>();
export function handleFlowMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  const workspace = String(args?.workspaceId ?? "");
  if (command === "flow_list")
    return [...definitions.values()].filter(
      (d) => d.workspaceId === workspace,
    ) as T;
  if (command === "flow_runs_list") return [] as T;
  if (command === "flow_save") {
    const input = structuredClone(args?.input as FlowDefinition);
    input.id ||= crypto.randomUUID();
    input.revision += 1;
    definitions.set(input.id, input);
    return structuredClone(input) as T;
  }
  if (command === "flow_get")
    return structuredClone(definitions.get(String(args?.flowId))) as T;
  if (command === "flow_delete") {
    const id = String(args?.flowId);
    if (definitions.get(id)?.workspaceId === workspace) definitions.delete(id);
    return undefined as T;
  }
  if (command.startsWith("flow_run"))
    throw {
      code: "FLOW_DESKTOP_REQUIRED",
      message: "Flow execution requires the desktop Rust runtime.",
    };
  return UNHANDLED;
}
