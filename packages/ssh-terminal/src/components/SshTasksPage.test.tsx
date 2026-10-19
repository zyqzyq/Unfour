// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { SshTaskRunEvent } from "@unfour/command-client";
import { SshTasksPage } from "./SshTasksPage";

const mocks = vi.hoisted(() => ({
  channelCleanup: vi.fn(),
  listener: null as ((event: SshTaskRunEvent) => void) | null,
  registerChannel: vi.fn((listener: (event: SshTaskRunEvent) => void) => {
    mocks.listener = listener;
    return Promise.resolve(mocks.channelCleanup);
  }),
}));

vi.mock("@unfour/command-client", () => ({
  cancelSshTaskRun: vi.fn(),
  clearSshTaskRuns: vi.fn(),
  deleteSshTask: vi.fn(),
  duplicateSshTask: vi.fn(),
  getSshTask: vi.fn(),
  listSshTaskRuns: vi.fn(async () => []),
  listSshTasks: vi.fn(async () => []),
  listWorkspaceEnvironments: vi.fn(async () => []),
  listWorkspaceVariables: vi.fn(async () => []),
  readSshTaskRunLog: vi.fn(async () => ""),
  registerSshTaskRunChannel: mocks.registerChannel,
  reorderSshTasks: vi.fn(),
  runSshTask: vi.fn(),
  saveSshTask: vi.fn(),
}));

vi.mock("@unfour/ui", () => ({
  ConfirmDialog: () => null,
  ErrorState: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SegmentedControl: () => null,
  SplitPane: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  Tabs: () => null,
  useFeedbackErrorHandler: () => vi.fn(),
  useFeedback: () => ({ info: vi.fn() }),
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("./TaskEditor", () => ({ TaskEditor: () => null }));
vi.mock("./TaskHistory", () => ({ TaskHistory: () => null }));
vi.mock("./TaskList", () => ({ TaskList: () => null }));
vi.mock("./TaskRunDialog", () => ({ TaskRunDialog: () => null }));
vi.mock("./TaskRunPanel", () => ({ TaskRunPanel: () => null }));
vi.mock("./TaskWorkspaceEmpty", () => ({ TaskWorkspaceEmpty: () => null }));

afterEach(() => {
  cleanup();
  vi.clearAllTimers();
  vi.useRealTimers();
  mocks.listener = null;
  mocks.channelCleanup.mockReset();
  mocks.registerChannel.mockClear();
});

describe("SshTasksPage event channel lifecycle", () => {
  it("does not leave a retry timer when a hidden page unmounts with pending events", async () => {
    vi.useFakeTimers();
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { gcTime: Infinity, retry: false },
        mutations: { retry: false },
      },
    });
    const view = render(
      <QueryClientProvider client={queryClient}>
        <SshTasksPage
          active={false}
          connections={[]}
          onOpenConnections={vi.fn()}
          workspaceId="workspace-one"
        />
      </QueryClientProvider>,
    );

    await act(async () => Promise.resolve());
    act(() => mocks.listener?.(taskEvent()));
    const timersBeforeUnmount = vi.getTimerCount();
    expect(timersBeforeUnmount).toBeGreaterThan(0);

    view.unmount();

    expect(vi.getTimerCount()).toBeLessThan(timersBeforeUnmount);
    expect(mocks.channelCleanup).toHaveBeenCalledOnce();
  });
});

function taskEvent(): SshTaskRunEvent {
  return {
    runId: "run-one",
    taskId: "task-one",
    kind: "output",
    stepId: "step-one",
    stepName: "Step",
    stepType: "command",
    position: 0,
    status: null,
    stream: "stdout",
    data: "pending output\n",
    exitCode: null,
    durationMs: null,
    direction: null,
    transferredBytes: null,
    totalBytes: null,
    bytesPerSecond: null,
    error: null,
    createdAt: "2026-08-11T00:00:00.000Z",
  };
}
