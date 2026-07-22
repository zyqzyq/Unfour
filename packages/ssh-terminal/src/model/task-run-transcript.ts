import type { SshTaskRunEvent } from "@unfour/command-client";
import { formatFileSize } from "./sftp-format";

export type TaskRunTranscriptLine = {
  key: string;
  kind: "header" | "output" | "meta" | "transfer" | "error";
  stream?: "stdout" | "stderr" | null;
  text: string;
};

/** Build a readable transcript from live task-run events. */
export function buildTaskRunTranscript(
  events: SshTaskRunEvent[],
  labels: {
    stepHeader: (position: number, name: string) => string;
    stepDone: (status: string, detail: string) => string;
    transfer: (direction: string, transferred: string, total: string, speed: string) => string;
  },
): TaskRunTranscriptLine[] {
  const lines: TaskRunTranscriptLine[] = [];
  let lastStepId: string | null = null;
  const lastTransferByStep = new Map<string, SshTaskRunEvent>();

  for (const [index, event] of events.entries()) {
    if (event.kind === "step" && event.status === "running" && event.stepId) {
      lastStepId = event.stepId;
      const position = (event.position ?? 0) + 1;
      const name = event.stepName?.trim() || event.stepId;
      lines.push({
        key: `header-${event.stepId}-${event.createdAt}-${index}`,
        kind: "header",
        text: labels.stepHeader(position, name),
      });
      continue;
    }

    if (event.kind === "output" && event.data) {
      if (event.stepId && event.stepId !== lastStepId) {
        lastStepId = event.stepId;
        const position = (event.position ?? 0) + 1;
        const name = event.stepName?.trim() || event.stepId;
        lines.push({
          key: `header-output-${event.stepId}-${index}`,
          kind: "header",
          text: labels.stepHeader(position, name),
        });
      }
      lines.push({
        key: `output-${event.createdAt}-${index}`,
        kind: "output",
        stream: event.stream,
        text: event.data,
      });
      continue;
    }

    if (event.kind === "transfer" && event.stepId) {
      lastTransferByStep.set(event.stepId, event);
      continue;
    }

    if (
      event.kind === "step" &&
      event.stepId &&
      event.status &&
      event.status !== "running"
    ) {
      const transfer = lastTransferByStep.get(event.stepId);
      if (transfer) {
        lines.push({
          key: `transfer-${event.stepId}-${index}`,
          kind: "transfer",
          text: labels.transfer(
            transfer.direction ?? "transfer",
            formatFileSize(transfer.transferredBytes ?? 0),
            formatFileSize(transfer.totalBytes ?? 0),
            `${formatFileSize(transfer.bytesPerSecond ?? 0)}/s`,
          ),
        });
        lastTransferByStep.delete(event.stepId);
      }

      const details: string[] = [];
      if (event.durationMs !== null && event.durationMs !== undefined) {
        details.push(`${event.durationMs} ms`);
      }
      if (event.exitCode !== null && event.exitCode !== undefined) {
        details.push(`exit ${event.exitCode}`);
      }
      lines.push({
        key: `done-${event.stepId}-${index}`,
        kind: "meta",
        text: labels.stepDone(event.status, details.join(" · ")),
      });
      if (event.error) {
        lines.push({
          key: `error-${event.stepId}-${index}`,
          kind: "error",
          text: event.error,
        });
      }
    }
  }

  // Still-running transfer: show latest progress under the open step.
  for (const [stepId, transfer] of lastTransferByStep) {
    lines.push({
      key: `transfer-live-${stepId}`,
      kind: "transfer",
      text: labels.transfer(
        transfer.direction ?? "transfer",
        formatFileSize(transfer.transferredBytes ?? 0),
        formatFileSize(transfer.totalBytes ?? 0),
        `${formatFileSize(transfer.bytesPerSecond ?? 0)}/s`,
      ),
    });
  }

  return lines;
}
