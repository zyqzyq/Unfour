import type { FlowDefinition } from "../../types/flow";
import { UNHANDLED, type MockResult } from "./types";

// Browser preview only supports authoring. Execution always requires Rust.
const definitions = new Map<string, FlowDefinition>();
const deleted = new Set<string>();
const conflict = () => ({ code: "FLOW_REVISION_CONFLICT", message: "FLOW_REVISION_CONFLICT" });

function atRevision(id: string, workspace: string, revision: unknown) {
  const current = definitions.get(id);
  if (!current || current.workspaceId !== workspace || deleted.has(id) || current.revision !== revision)
    throw conflict();
  return current;
}

function saveDefinition(input: FlowDefinition) {
  if (input.id) {
    atRevision(input.id, input.workspaceId, input.revision);
    input.revision += 1;
  } else {
    input.id = crypto.randomUUID();
    input.revision = 1;
  }
  definitions.set(input.id, input);
  return structuredClone(input);
}

function getDefinition(workspace: string, id: string) {
  const current = definitions.get(id);
  if (!current || current.workspaceId !== workspace || deleted.has(id))
    throw { code: "NOT_FOUND", message: "flow" };
  return structuredClone(current);
}
export function handleFlowMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  const workspace = String(args?.workspaceId ?? "");
  if (command === "flow_list")
    return [...definitions.values()].filter(
      (d) => d.workspaceId === workspace && !deleted.has(d.id),
    ) as T;
  if (command === "flow_runs_list") return [] as T;
  if (command === "flow_save") {
    const input = structuredClone(args?.input as FlowDefinition);
    return saveDefinition(input) as T;
  }
  if (command === "flow_get") {
    return getDefinition(workspace, String(args?.flowId)) as T;
  }
  if (command === "flow_delete") {
    const id = String(args?.flowId);
    const current = atRevision(id, workspace, args?.expectedRevision);
    current.revision += 1;
    deleted.add(id);
    return undefined as T;
  }
  if (command.startsWith("flow_run"))
    throw {
      code: "FLOW_DESKTOP_REQUIRED",
      message: "Flow execution requires the desktop Rust runtime.",
    };
  return UNHANDLED;
}
