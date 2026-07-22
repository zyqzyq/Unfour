import { describe, expect, it } from "vitest";
import type { SshTaskRunEvent } from "@unfour/command-client";
import { buildTaskRunTranscript } from "./task-run-transcript";

function event(partial: Partial<SshTaskRunEvent>): SshTaskRunEvent {
  return {
    runId: "run-1",
    taskId: "task-1",
    kind: "output",
    stepId: null,
    stepName: null,
    stepType: null,
    position: null,
    status: null,
    stream: null,
    data: null,
    exitCode: null,
    durationMs: null,
    direction: null,
    transferredBytes: null,
    totalBytes: null,
    bytesPerSecond: null,
    error: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    ...partial,
  };
}

const labels = {
  stepHeader: (position: number, name: string) => `── ${position}. ${name} ──`,
  stepDone: (status: string, detail: string) =>
    detail ? `↳ ${status} · ${detail}` : `↳ ${status}`,
  transfer: (direction: string, transferred: string, total: string, speed: string) =>
    `${direction}: ${transferred} / ${total} (${speed})`,
};

describe("buildTaskRunTranscript", () => {
  it("segments command output by step and appends completion meta", () => {
    const lines = buildTaskRunTranscript(
      [
        event({
          kind: "step",
          stepId: "s1",
          stepName: "Build",
          position: 0,
          status: "running",
        }),
        event({
          kind: "output",
          stepId: "s1",
          stepName: "Build",
          position: 0,
          stream: "stdout",
          data: "compiling...\n",
        }),
        event({
          kind: "step",
          stepId: "s1",
          stepName: "Build",
          position: 0,
          status: "success",
          durationMs: 120,
          exitCode: 0,
        }),
      ],
      labels,
    );

    expect(lines.map((line) => line.text)).toEqual([
      "── 1. Build ──",
      "compiling...\n",
      "↳ success · 120 ms · exit 0",
    ]);
    expect(lines[1]?.stream).toBe("stdout");
  });

  it("summarizes transfer steps once when the step finishes", () => {
    const lines = buildTaskRunTranscript(
      [
        event({
          kind: "step",
          stepId: "up",
          stepName: "Upload artifact",
          position: 1,
          status: "running",
          stepType: "upload",
        }),
        event({
          kind: "transfer",
          stepId: "up",
          direction: "upload",
          transferredBytes: 512,
          totalBytes: 1024,
          bytesPerSecond: 256,
        }),
        event({
          kind: "transfer",
          stepId: "up",
          direction: "upload",
          transferredBytes: 1024,
          totalBytes: 1024,
          bytesPerSecond: 512,
        }),
        event({
          kind: "step",
          stepId: "up",
          stepName: "Upload artifact",
          position: 1,
          status: "success",
          durationMs: 40,
        }),
      ],
      labels,
    );

    expect(lines.map((line) => line.kind)).toEqual(["header", "transfer", "meta"]);
    expect(lines[1]?.text).toContain("upload:");
    expect(lines[1]?.text).toContain("1.0 KB / 1.0 KB");
  });
});
