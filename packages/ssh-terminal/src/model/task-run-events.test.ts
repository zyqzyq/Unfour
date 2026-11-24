import { describe, expect, it } from "vitest";
import type { SshTaskRunEvent } from "@unfour/command-client";
import {
  type LiveTaskRunEvent,
  MAX_CACHED_TASK_LOGS,
  MAX_CACHED_TASK_RUNS,
  appendTaskRunEventCache,
  appendTaskRunEvents,
  cacheTaskRunLog,
  removeTaskRunEventsForTask,
} from "./task-run-events";

let sequence = 0;
function event(partial: Partial<SshTaskRunEvent>): LiveTaskRunEvent {
  return {
    sequence: sequence++,
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

describe("appendTaskRunEvents", () => {
  it("keeps only the latest transfer progress for each step", () => {
    const retained = appendTaskRunEvents(
      [
        event({ kind: "step", stepId: "download", status: "running" }),
        event({
          kind: "transfer",
          stepId: "download",
          transferredBytes: 256,
          totalBytes: 1_024,
        }),
      ],
      [
        event({
          kind: "transfer",
          stepId: "download",
          transferredBytes: 768,
          totalBytes: 1_024,
        }),
        event({
          kind: "transfer",
          stepId: "download",
          transferredBytes: 1_024,
          totalBytes: 1_024,
        }),
        event({ kind: "step", stepId: "download", status: "success" }),
      ],
    );

    expect(retained.map((item) => item.kind)).toEqual(["step", "transfer", "step"]);
    expect(retained[1]?.transferredBytes).toBe(1_024);
  });

  it("bounds retained non-progress events", () => {
    const retained = appendTaskRunEvents(
      [],
      Array.from({ length: 5_100 }, (_, index) =>
        event({ data: `${index}\n`, createdAt: `event-${index}` }),
      ),
    );

    expect(retained).toHaveLength(5_000);
    expect(retained[0]?.data).toBe("100\n");
    expect(retained.at(-1)?.data).toBe("5099\n");
  });

  it("bounds cached run ids while preserving the active run", () => {
    const current = Object.fromEntries(
      Array.from({ length: MAX_CACHED_TASK_RUNS }, (_, index) => [
        `run-${index}`,
        [event({ runId: `run-${index}`, createdAt: `event-${index}` })],
      ]),
    );

    const retained = appendTaskRunEventCache(
      current,
      [event({ runId: "run-new", createdAt: "event-new" })],
      "run-0",
    );

    expect(Object.keys(retained)).toHaveLength(MAX_CACHED_TASK_RUNS);
    expect(retained["run-0"]).toBeDefined();
    expect(retained["run-1"]).toBeUndefined();
    expect(retained["run-new"]).toBeDefined();
  });

  it("keeps only a small LRU of history log strings", () => {
    let cache: Record<string, string> = {};
    for (let index = 0; index <= MAX_CACHED_TASK_LOGS; index += 1) {
      cache = cacheTaskRunLog(cache, `run-${index}`, `log-${index}`);
    }

    expect(Object.keys(cache)).toHaveLength(MAX_CACHED_TASK_LOGS);
    expect(cache["run-0"]).toBeUndefined();
    expect(cache[`run-${MAX_CACHED_TASK_LOGS}`]).toBe(
      `log-${MAX_CACHED_TASK_LOGS}`,
    );
  });

  it("clears only cached runs for the selected task", () => {
    const retained = removeTaskRunEventsForTask(
      {
        "run-one": [event({ runId: "run-one", taskId: "task-one" })],
        "run-two": [event({ runId: "run-two", taskId: "task-two" })],
      },
      "task-one",
    );

    expect(retained["run-one"]).toBeUndefined();
    expect(retained["run-two"]).toBeDefined();
  });
});
