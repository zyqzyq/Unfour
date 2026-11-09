// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useDesktopWorkspace } from "./useDesktopWorkspace";

vi.mock("./useWorkspaceInit", () => ({ useWorkspaceInit: () => {} }));
vi.mock("./useLayoutPersistence", () => ({ useLayoutPersistence: () => {} }));
vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  getSystemHealth: vi.fn().mockResolvedValue({}), getWorkspaceLayout: vi.fn().mockResolvedValue({}),
  listDatabaseConnections: vi.fn().mockResolvedValue([]), getWorkspaceState: vi.fn(),
  listWorkspaceEnvironments: vi.fn(), setActiveWorkspaceEnvironment: vi.fn(),
}));
const env = (workspaceId: string, id: string): commands.WorkspaceEnvironment => ({
  workspaceId, id, name: id, isActive: true, variables: [], revision: 1, sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null,
});
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const hook = renderHook(() => useDesktopWorkspace(), { wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> });
  return { ...hook, client };
}
beforeEach(() => {
  vi.clearAllMocks();
  useWorkspaceStore.getState().setActiveWorkspace("a");
  vi.mocked(commands.getWorkspaceState).mockResolvedValue({ activeWorkspaceId: "a", workspaces: ["a", "b"].map((id) => ({ id, name: id, isDefault: id === "a", environmentType: "test", mcpPolicy: "auto", createdAt: "", updatedAt: "", lastOpenedAt: null, deletedAt: null, revision: 1 })) });
  vi.mocked(commands.listWorkspaceEnvironments).mockImplementation(async (id) => [env(id, id + "1")]);
});
afterEach(cleanup);

it("serializes rapid selections, keeps pending through the queue and scopes late results to their workspace", async () => {
  const completions: ((value: commands.WorkspaceEnvironment[]) => void)[] = [];
  vi.mocked(commands.setActiveWorkspaceEnvironment).mockImplementation(() => new Promise((resolve) => completions.push(resolve)));
  const { result, client } = setup();
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("a1"));
  act(() => { result.current.selectEnvironment("a2"); result.current.selectEnvironment("a3"); });
  await waitFor(() => expect(completions).toHaveLength(1));
  expect(result.current.environmentSwitching).toBe(true);
  await act(async () => completions[0]([env("a", "a2")]));
  await waitFor(() => expect(completions).toHaveLength(2));
  expect(result.current.environmentSwitching).toBe(true);
  act(() => useWorkspaceStore.getState().setActiveWorkspace("b"));
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("b1"));
  expect(result.current.environmentSwitching).toBe(false);
  await act(async () => completions[1]([env("a", "a3")]));
  expect(result.current.activeEnvironment?.id).toBe("b1");
  expect(client.getQueryData(["workspace-environments", "a"])).toEqual([env("a", "a3")]);
  expect(commands.setActiveWorkspaceEnvironment).toHaveBeenNthCalledWith(2, "a", "a3");
  client.clear();
});

it("does not allow an older list response to overwrite a completed activation", async () => {
  const { result, client } = setup();
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("a1"));
  let finishList!: (value: commands.WorkspaceEnvironment[]) => void;
  vi.mocked(commands.listWorkspaceEnvironments).mockImplementationOnce(() => new Promise((resolve) => { finishList = resolve; }));
  act(() => { void client.refetchQueries({ queryKey: ["workspace-environments", "a"] }); });
  vi.mocked(commands.setActiveWorkspaceEnvironment).mockResolvedValueOnce([env("a", "a2")]);
  act(() => result.current.selectEnvironment("a2"));
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("a2"));
  await act(async () => finishList([env("a", "a1")]));
  expect(result.current.activeEnvironment?.id).toBe("a2");
  client.clear();
});

it("continues queued choices after failure and keeps different workspace queues independent", async () => {
  let failFirst!: (error: Error) => void;
  let finishSecond!: (value: commands.WorkspaceEnvironment[]) => void;
  vi.mocked(commands.setActiveWorkspaceEnvironment)
    .mockImplementationOnce(() => new Promise((_resolve, reject) => { failFirst = reject; }))
    .mockImplementationOnce(() => new Promise((resolve) => { finishSecond = resolve; }))
    .mockResolvedValueOnce([env("b", "b2")]);
  const { result, client } = setup();
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("a1"));
  act(() => { result.current.selectEnvironment("a2"); result.current.selectEnvironment("a3"); });
  await waitFor(() => expect(commands.setActiveWorkspaceEnvironment).toHaveBeenCalledTimes(1));
  await act(async () => failFirst(new Error("isolated failure")));
  await waitFor(() => expect(commands.setActiveWorkspaceEnvironment).toHaveBeenCalledTimes(2));
  expect(result.current.activeEnvironment?.id).toBe("a1");
  expect(result.current.environmentSwitching).toBe(true);
  act(() => useWorkspaceStore.getState().setActiveWorkspace("b"));
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("b1"));
  act(() => result.current.selectEnvironment("b2"));
  await waitFor(() => expect(result.current.activeEnvironment?.id).toBe("b2"));
  await act(async () => finishSecond([env("a", "a3")]));
  expect(result.current.activeEnvironment?.id).toBe("b2");
  expect(client.getQueryData(["workspace-environments", "a"])).toEqual([env("a", "a3")]);
  client.clear();
});
