export type FlowCapability = "api" | "ssh" | "database";
export type FlowAction = {
  capability: FlowCapability;
  resourceId: string;
  connectionId: string | null;
  arguments: Record<string, unknown>;
};
export type FlowPredicate = {
  left: unknown;
  op: "eq" | "ne" | "gt" | "ge" | "lt" | "le";
  right: unknown;
};
export type FlowStep = {
  id: string;
  name: string;
  timeoutMs: number;
  next: string | null;
} & (
  | { kind: "action"; action: FlowAction }
  | {
      kind: "condition";
      predicate: FlowPredicate;
      ifTrue: string;
      ifFalse: string;
    }
  | {
      kind: "poll";
      probe: FlowAction;
      predicate: FlowPredicate;
      intervalMs: number;
      maxAttempts: number;
    }
  | { kind: "wait"; durationMs: number }
);
export type FlowDefinition = {
  id: string;
  workspaceId: string;
  name: string;
  revision: number;
  inputs: string[];
  steps: FlowStep[];
};
export type FlowRunInput = {
  workspaceId: string;
  flowId: string;
  environmentId: string | null;
  inputs: Record<string, unknown>;
  secretInputNames?: string[];
  initiator: "human" | "mcp";
  confirmEffects: boolean;
};
export type FlowAttempt = {
  number: number;
  input: unknown;
  output: unknown | null;
  error: string | null;
  durationMs: number;
};
export type FlowStepRun = {
  stepId: string;
  status: string;
  durationMs: number;
  attempts: FlowAttempt[];
  error: string | null;
};
export type FlowRun = {
  id: string;
  workspaceId: string;
  flowId: string;
  definition: FlowDefinition;
  context: FlowRunInput;
  resources: unknown;
  status: string;
  error: string | null;
  startedAt: string;
  finishedAt: string | null;
  steps: FlowStepRun[];
};
