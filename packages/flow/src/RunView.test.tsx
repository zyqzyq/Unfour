// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { RunView } from "./RunView";
import type { FlowRun } from "@unfour/command-client";
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("retains the last result and error while a new check is in flight, with live elapsed time", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-20T00:00:02Z"));
  const run: FlowRun = {
    id: "r", workspaceId: "w", flowId: "f", status: "running", startedAt: "2026-09-20T00:00:00Z", finishedAt: null, error: null, resources: {},
    context: { workspaceId: "w", flowId: "f", environmentId: null, inputs: {}, initiator: "human", confirmEffects: true },
    definition: { id: "f", name: "Flow", workspaceId: "w", revision: 1, inputs: [], steps: [] },
    steps: [{ stepId: "s", status: "running", startedAt: "2026-09-20T00:00:00Z", durationMs: 0, error: null, attempts: [
      { number: 1, input: {}, output: { state: "pending" }, error: null, durationMs: 5 },
      { number: 2, input: {}, output: null, error: "Temporary failure", durationMs: 5 },
      { number: 3, input: {}, output: null, error: null, durationMs: 0 },
    ] }],
  };
  render(<I18nProvider initialLocale="en"><RunView run={run} cancel={() => {}} /></I18nProvider>);
  expect(screen.getByText("Latest result").nextElementSibling).toHaveTextContent('"state": "pending"');
  expect(screen.getByText(/Latest error/)).toHaveTextContent("2");
  expect(screen.getByText(/Elapsed: 2000 ms/)).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(1000));
  expect(screen.getByText(/Elapsed: 3000 ms/)).toBeInTheDocument();
});
