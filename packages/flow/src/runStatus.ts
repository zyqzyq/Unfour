import type { FlowRunStatus, FlowStepRunStatus } from "@unfour/command-client";
import type { TFunction } from "@unfour/ui";

const outcomeUncertain = "FLOW_OUTCOME_UNCERTAIN";

export function flowDisplayStatus(run: {
  status: FlowRunStatus | FlowStepRunStatus;
  error?: string | null;
  outcomeUncertain?: boolean;
}) {
  if (run.status === "failed" && (run.error === outcomeUncertain || run.outcomeUncertain)) {
    return "outcomeUncertain";
  }
  return run.status;
}

export function formatFlowRunError(error: string, t: TFunction) {
  return error === outcomeUncertain ? t("flow.outcomeUncertainHint") : error;
}
