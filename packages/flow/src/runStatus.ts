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
  if (error === "FLOW_HISTORY_WRITE_FAILED") return t("flow.historyWriteFailedHint");
  if (error === "FLOW_OUTPUT_TOO_LARGE" || error === "FLOW_HISTORY_LIMIT") return t("flow.resultLimitHint");
  return error === outcomeUncertain ? t("flow.outcomeUncertainHint") : error;
}
