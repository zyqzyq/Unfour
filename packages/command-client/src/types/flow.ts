export type FlowCapability = "api" | "ssh" | "database";
export type FlowAction = {
  capability: FlowCapability;
  resourceId: string;
  connectionId: string | null;
  arguments: Record<string, unknown>;
};
export type FlowPredicate = {
  left: unknown;
  op: FlowOperator;
  right: unknown;
};
export type FlowOperator = "eq" | "ne" | "gt" | "ge" | "lt" | "le" | "in";
export type FlowInputDefinition = {
  name: string;
  type: "string" | "number" | "boolean" | "json";
  required: boolean;
  default?: unknown;
  secret: boolean;
  description?: string;
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
  | {
      kind: "waitUntil";
      probe: FlowAction;
      successWhen: FlowPredicate;
      failureWhen?: FlowPredicate | null;
      intervalMs: number;
      maxAttempts?: number | null;
      probeErrorPolicy: "failImmediately" | "retryTransientErrors";
      intervalStrategy: "fixed";
    }
);
export type FlowDefinition = {
  id: string;
  workspaceId: string;
  name: string;
  revision: number;
  inputs: FlowInputDefinition[];
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
  status: FlowStepRunStatus;
  startedAt?: string | null;
  nextCheckAt?: string | null;
  output?: unknown | null;
  durationMs: number;
  attempts: FlowAttempt[];
  error: string | null;
};
export type FlowRunStatus = "running" | "succeeded" | "failed" | "timedOut" | "cancelled" | "interrupted" | "validationFailed";
export type FlowStepRunStatus = "pending" | "running" | "succeeded" | "failed" | "timedOut" | "cancelled" | "interrupted" | "skipped";
export type FlowRunSummary = Pick<FlowRun, "id" | "flowId" | "status" | "startedAt" | "finishedAt">;

export type FlowRun = {
  id: string;
  workspaceId: string;
  flowId: string;
  definition: FlowDefinition;
  context: FlowRunInput;
  resources: unknown;
  status: FlowRunStatus;
  error: string | null;
  startedAt: string;
  finishedAt: string | null;
  steps: FlowStepRun[];
};
