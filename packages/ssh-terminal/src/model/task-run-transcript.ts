import type { SshTaskRunEvent } from "@unfour/command-client";
import { formatFileSize } from "./sftp-format";

export type TaskRunTranscriptLine = {
  key: string;
  kind: "header" | "command" | "output" | "meta" | "transfer" | "error";
  stream?: "stdout" | "stderr" | "command" | null;
  text: string;
};

type TranscriptLabels = {
  stepHeader: (position: number, name: string) => string;
  stepDone: (status: string, detail: string) => string;
  transfer: (direction: string, transferred: string, total: string, speed: string) => string;
};

type TranscriptState = {
  lines: TaskRunTranscriptLine[];
  lastStepId: string | null;
  lastTransferByStep: Map<string, SshTaskRunEvent>;
};

function headerLine(
  event: SshTaskRunEvent,
  stepId: string,
  key: string,
  labels: TranscriptLabels,
): TaskRunTranscriptLine {
  return {
    key,
    kind: "header",
    text: labels.stepHeader((event.position ?? 0) + 1, event.stepName?.trim() || stepId),
  };
}

function transferLine(
  event: SshTaskRunEvent,
  key: string,
  labels: TranscriptLabels,
): TaskRunTranscriptLine {
  return {
    key,
    kind: "transfer",
    text: labels.transfer(
      event.direction ?? "transfer",
      formatFileSize(event.transferredBytes ?? 0),
      formatFileSize(event.totalBytes ?? 0),
      `${formatFileSize(event.bytesPerSecond ?? 0)}/s`,
    ),
  };
}

function stepDetails(event: SshTaskRunEvent): string {
  const details: string[] = [];
  if (event.durationMs !== null && event.durationMs !== undefined) {
    details.push(`${event.durationMs} ms`);
  }
  if (event.exitCode !== null && event.exitCode !== undefined) {
    details.push(`exit ${event.exitCode}`);
  }
  return details.join(" · ");
}

function appendStep(
  state: TranscriptState,
  event: SshTaskRunEvent,
  index: number,
  labels: TranscriptLabels,
) {
  if (!event.stepId || !event.status) return;
  if (event.status === "running") {
    state.lastStepId = event.stepId;
    state.lines.push(
      headerLine(event, event.stepId, `header-${event.stepId}-${event.createdAt}-${index}`, labels),
    );
    return;
  }

  const transfer = state.lastTransferByStep.get(event.stepId);
  if (transfer) {
    state.lines.push(transferLine(transfer, `transfer-${event.stepId}-${index}`, labels));
    state.lastTransferByStep.delete(event.stepId);
  }
  state.lines.push({
    key: `done-${event.stepId}-${index}`,
    kind: "meta",
    text: labels.stepDone(event.status, stepDetails(event)),
  });
  if (event.error) {
    state.lines.push({ key: `error-${event.stepId}-${index}`, kind: "error", text: event.error });
  }
}

function appendOutput(
  state: TranscriptState,
  event: SshTaskRunEvent,
  index: number,
  labels: TranscriptLabels,
) {
  if (!event.data) return;
  if (event.stepId && event.stepId !== state.lastStepId) {
    state.lastStepId = event.stepId;
    state.lines.push(headerLine(event, event.stepId, `header-output-${event.stepId}-${index}`, labels));
  }
  state.lines.push({
    key: `output-${event.createdAt}-${index}`,
    kind: event.stream === "command" ? "command" : "output",
    stream: event.stream,
    text: event.data,
  });
}

/** Build a readable transcript from live task-run events, in received order. */
export function buildTaskRunTranscript(
  events: SshTaskRunEvent[],
  labels: TranscriptLabels,
): TaskRunTranscriptLine[] {
  const state: TranscriptState = {
    lines: [],
    lastStepId: null,
    lastTransferByStep: new Map(),
  };
  for (const [index, event] of events.entries()) {
    switch (event.kind) {
      case "step":
        appendStep(state, event, index, labels);
        break;
      case "output":
        appendOutput(state, event, index, labels);
        break;
      case "transfer":
        if (event.stepId) state.lastTransferByStep.set(event.stepId, event);
        break;
    }
  }
  // Unfinished transfers retain first-seen step order and latest progress.
  for (const [stepId, transfer] of state.lastTransferByStep) {
    state.lines.push(transferLine(transfer, `transfer-live-${stepId}`, labels));
  }
  return state.lines;
}
