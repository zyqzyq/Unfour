import type { SshTaskRunEvent, SshTaskStep } from "@unfour/command-client";
import type { TaskRunTranscriptLine } from "./task-run-transcript";

type RunStep = Pick<SshTaskStep, "id" | "position" | "name">;
type StepMarker = RunStep & { status: string };

// See ssh-engine/task/log_format.rs. Only physical LF-delimited metadata
// records in a versioned log carry identities. Legacy logs are plain text.
const header = "Unfour SSH task log v2\n";
const states = ["running", "success", "failed", "cancelled", "outcomeUncertain"];

function parseMarker(line: string): StepMarker | undefined {
  try {
    const value = JSON.parse(line.slice("@step ".length));
    if (value && typeof value.id === "string" && value.id && typeof value.name === "string" &&
        Number.isSafeInteger(value.position) && value.position >= 0 && states.includes(value.status)) return value;
  } catch {
    // The size cap may have cut this metadata record mid-write.
  }
  return undefined;
}

export function historyTranscript(log: string, formatText: (text: string) => string): TaskRunTranscriptLine[] {
  if (!log) return [];
  if (!log.startsWith(header)) return [{ key: "legacy-log", kind: "output", text: log }];
  const lines: TaskRunTranscriptLine[] = [];
  let marker: StepMarker | undefined;
  let text = "";
  function flush() {
    if (!text) return;
    // A bounded read can end immediately after the marker. Only offer a
    // jump if its corresponding readable step record is also present.
    const step = marker && /^\[[^\]\r\n]+\] step /.test(text) ? marker : undefined;
    lines.push({
      key: `log-${lines.length}`,
      kind: "output",
      text,
      stepId: step?.id,
      recordedStep: step,
      recordedStatus: step?.status,
    });
    text = "";
  }
  for (const [line] of log.slice(header.length).matchAll(/[^\n]*\n|[^\n]+$/g)) {
    if (line.startsWith("@step ")) {
      flush();
      marker = parseMarker(line);
    } else if (line.startsWith("@partial ")) {
      // Display event boundaries even when a chunk had no trailing newline.
      text += line.slice("@partial ".length);
    } else if (line.startsWith("@diagnostic ")) {
      text += formatText(line.slice("@diagnostic ".length));
    } else {
      text += line.startsWith("@@") ? line.slice(1) : line;
    }
  }
  flush();
  return lines;
}

export function runStepList(planned: readonly SshTaskStep[], events: readonly SshTaskRunEvent[], lines: readonly TaskRunTranscriptLine[]): RunStep[] {
  const observed = new Map<string, RunStep>();
  for (const event of events) {
    if (event.stepId && event.stepName != null && event.position != null && !observed.has(event.stepId)) {
      observed.set(event.stepId, { id: event.stepId, name: event.stepName, position: event.position });
    }
  }
  for (const { recordedStep } of lines) {
    if (recordedStep) observed.set(recordedStep.id, recordedStep);
  }
  for (const step of planned) {
    if (step.enabled && !observed.has(step.id)) observed.set(step.id, step);
  }
  return [...observed.values()].sort((left, right) => left.position - right.position);
}
