// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setActiveWorkspace, type WorkspaceState } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useDesktopNavigation } from "./useDesktopNavigation";

const feedback = vi.hoisted(() => ({ handleError: vi.fn() }));
vi.mock("@unfour/ui", () => ({ useFeedbackErrorHandler: () => feedback.handleError }));
vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(), setActiveWorkspace: vi.fn(),
}));

const previousState: WorkspaceState = { activeWorkspaceId: "one", workspaces: [{
  id: "one", name: "One", isDefault: true, environmentType: "dev", mcpPolicy: "auto",
  createdAt: "", updatedAt: "", deletedAt: null, lastOpenedAt: null, revision: 1,
}] };
function mount() {
  const queryClient = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  queryClient.setQueryData(["workspaces"], previousState);
  const setActiveTab = vi.fn();
  const preloadFeature = vi.fn().mockResolvedValue(undefined);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  const hook = renderHook(() => useDesktopNavigation({
    activeWorkspace: previousState.workspaces[0], activeEnvironmentId: "dev",
    tabs: [{ id: "ssh-main", kind: "ssh", title: "SSH" }], setActiveTab, preloadFeature,
  }), { wrapper });
  return { ...hook, queryClient, setActiveTab, preloadFeature };
}
beforeEach(() => {
  vi.clearAllMocks();
  useWorkspaceStore.getState().setActiveWorkspace("one");
});
afterEach(cleanup);

it("keeps dirty variables until workspace leave is confirmed, including after cancellation", async () => {
  vi.mocked(setActiveWorkspace).mockResolvedValue({ ...previousState, activeWorkspaceId: "two" });
  const { result, queryClient } = mount();
  act(() => result.current.handleManageVariables());
  const request = result.current.variableManagerRequest;
  act(() => result.current.setVariableManagerDirty(true));
  act(() => result.current.handleActivateWorkspace("two"));
  expect(result.current.variableManagerOpen).toBe(true);
  expect(setActiveWorkspace).not.toHaveBeenCalled();
  act(() => result.current.cancelVariableManagerLeave());
  act(() => result.current.handleManageVariables());
  expect(result.current.variableManagerRequest).toBe(request);
  act(() => result.current.handleActivateWorkspace("two"));
  act(() => result.current.confirmVariableManagerLeave());
  await waitFor(() => expect(queryClient.getQueryData<WorkspaceState>(["workspaces"])?.activeWorkspaceId).toBe("two"));
  expect(setActiveWorkspace).toHaveBeenCalledTimes(1);
  expect(result.current.variableManagerOpen).toBe(false);
  expect(result.current.pendingVariableManagerLeave).toBeNull();
  queryClient.clear();
});

it.each([false, true])("preserves optimistic workspace activation and rollback (failure=%s)", async (failure) => {
  let resolve!: (value: WorkspaceState) => void;
  let reject!: (reason: Error) => void;
  vi.mocked(setActiveWorkspace).mockReturnValue(new Promise((done, fail) => { resolve = done; reject = fail; }));
  const { result, queryClient } = mount();
  act(() => result.current.handleActivateWorkspace("one"));
  expect(setActiveWorkspace).not.toHaveBeenCalled();
  act(() => result.current.handleActivateWorkspace("two"));
  await waitFor(() => expect(useWorkspaceStore.getState().activeWorkspaceId).toBe("two"));
  expect(queryClient.getQueryData<WorkspaceState>(["workspaces"])?.activeWorkspaceId).toBe("two");
  act(() => result.current.handleActivateWorkspace("three"));
  expect(setActiveWorkspace).toHaveBeenCalledTimes(1);
  await act(async () => {
    if (failure) reject(new Error("activation failed"));
    else resolve({ ...previousState, activeWorkspaceId: "two" });
  });
  await waitFor(() => expect(useWorkspaceStore.getState().activeWorkspaceId).toBe(failure ? "one" : "two"));
  expect(queryClient.getQueryData<WorkspaceState>(["workspaces"])?.activeWorkspaceId).toBe(failure ? "one" : "two");
  expect(feedback.handleError).toHaveBeenCalledTimes(failure ? 1 : 0);
  queryClient.clear();
});

it("preloads a module while keeping the dirty manager until its leave is confirmed", () => {
  const { result, queryClient, setActiveTab, preloadFeature } = mount();
  act(() => result.current.handleManageVariables());
  act(() => result.current.setVariableManagerDirty(true));
  act(() => result.current.handleSelectModule("ssh-main"));
  expect(preloadFeature).toHaveBeenCalledWith("ssh");
  expect(setActiveTab).not.toHaveBeenCalled();
  act(() => result.current.confirmVariableManagerLeave());
  expect(setActiveTab).toHaveBeenCalledWith("ssh-main");
  expect(result.current.variableManagerOpen).toBe(false);
  queryClient.clear();
});
