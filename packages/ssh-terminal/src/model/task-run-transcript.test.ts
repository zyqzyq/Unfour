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
          stream: "command",
          data: "$ cargo build\n",
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
      "$ cargo build\n",
      "compiling...\n",
      "↳ success · 120 ms · exit 0",
    ]);
    expect(lines[1]?.kind).toBe("command");
    expect(lines[2]?.stream).toBe("stdout");
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

describe("transcript event ordering and incomplete events", () => {
  it("keeps input order, implicit headers, streams and stable keys", () => {
    const events = [
      event({ stepId: "a", stepName: "  Build  ", position: 2, data: "first", stream: "stderr", createdAt: "later" }),
      event({ data: "global", createdAt: "earlier" }),
      event({ stepId: "a", data: "same" }),
      event({ stepId: "b", stepName: " ", data: "other", stream: "command" }),
      event({ stepId: "a", data: "back" }),
    ];
    const before = structuredClone(events);
    const lines = buildTaskRunTranscript(events, labels);
    expect(lines).toEqual([
      { key: "header-output-a-0", kind: "header", text: "── 3. Build ──" },
      { key: "output-later-0", kind: "output", stream: "stderr", text: "first" },
      { key: "output-earlier-1", kind: "output", stream: null, text: "global" },
      { key: `output-${events[2].createdAt}-2`, kind: "output", stream: null, text: "same" },
      { key: "header-output-b-3", kind: "header", text: "── 1. b ──" },
      { key: `output-${events[3].createdAt}-3`, kind: "command", stream: "command", text: "other" },
      { key: "header-output-a-4", kind: "header", text: "── 1. a ──" },
      { key: `output-${events[4].createdAt}-4`, kind: "output", stream: null, text: "back" },
    ]);
    expect(events).toEqual(before);
    expect(buildTaskRunTranscript(events, labels)).toEqual(lines);
  });

  it("retains repeated running headers and ignores non-renderable events", () => {
    const running = event({ kind: "step", stepId: "a", status: "running", error: "ignored" });
    const lines = buildTaskRunTranscript([
      event({ kind: "run", status: "failed", error: "ignored" }),
      event({ kind: "step", status: "running" }),
      event({ kind: "step", stepId: "a" }),
      event({ kind: "transfer" }),
      event({ stepId: "a", data: "" }),
      running, running,
      event({ stepId: "a", data: " " }),
    ], labels);
    expect(lines.map((line) => line.kind)).toEqual(["header", "header", "output"]);
    expect(lines.map((line) => line.key)).toEqual([
      `header-a-${running.createdAt}-5`, `header-a-${running.createdAt}-6`,
      `output-${running.createdAt}-7`,
    ]);
  });

  it.each(["success", "failed", "cancelled"] as const)("flushes transfers before %s metadata and errors", (status) => {
    const lines = buildTaskRunTranscript([
      event({ kind: "transfer", stepId: "a" }),
      event({ kind: "step", stepId: "a", status, durationMs: 0, exitCode: 0, error: "failure detail" }),
      event({ kind: "step", stepId: "a", status }),
    ], labels);
    expect(lines).toEqual([
      { key: "transfer-a-1", kind: "transfer", text: "transfer: 0 B / 0 B (0 B/s)" },
      { key: "done-a-1", kind: "meta", text: `↳ ${status} · 0 ms · exit 0` },
      { key: "error-a-1", kind: "error", text: "failure detail" },
      { key: "done-a-2", kind: "meta", text: `↳ ${status}` },
    ]);
  });

  it("keeps latest live transfers in first-seen order and accepts late progress after completion", () => {
    const lines = buildTaskRunTranscript([
      event({ kind: "transfer", stepId: "b", direction: "download", transferredBytes: 1 }),
      event({ kind: "transfer", stepId: "a", direction: "upload", transferredBytes: 2 }),
      event({ kind: "transfer", stepId: "b", direction: "download", transferredBytes: 3 }),
      event({ kind: "step", stepId: "b", status: "success" }),
      event({ kind: "transfer", stepId: "b", transferredBytes: 4 }),
      event({ kind: "transfer", stepId: "a", transferredBytes: 5 }),
    ], labels);
    expect(lines.map(({ key, text }) => [key, text])).toEqual([
      ["transfer-b-3", "download: 3 B / 0 B (0 B/s)"],
      ["done-b-3", "↳ success"],
      ["transfer-live-a", "transfer: 5 B / 0 B (0 B/s)"],
      ["transfer-live-b", "transfer: 4 B / 0 B (0 B/s)"],
    ]);
  });
});
