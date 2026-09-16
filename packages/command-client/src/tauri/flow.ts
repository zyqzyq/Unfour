import { call } from "./invoke";
import type { FlowDefinition, FlowRun, FlowRunInput } from "../types/flow";
export const listFlows = (workspaceId: string) =>
  call<FlowDefinition[]>("flow_list", { workspaceId });
export const getFlow = (workspaceId: string, flowId: string) =>
  call<FlowDefinition>("flow_get", { workspaceId, flowId });
export const saveFlow = (input: FlowDefinition) =>
  call<FlowDefinition>("flow_save", { input });
export const deleteFlow = (workspaceId: string, flowId: string) =>
  call<void>("flow_delete", { workspaceId, flowId });
export const runFlow = (input: FlowRunInput) =>
  call<FlowRun>("flow_run", { input });
export const listFlowRuns = (workspaceId: string, flowId: string) =>
  call<FlowRun[]>("flow_runs_list", { workspaceId, flowId });
export const getFlowRun = (workspaceId: string, runId: string) =>
  call<FlowRun>("flow_run_get", { workspaceId, runId });
export const cancelFlowRun = (workspaceId: string, runId: string) =>
  call<FlowRun>("flow_run_cancel", { workspaceId, runId });
