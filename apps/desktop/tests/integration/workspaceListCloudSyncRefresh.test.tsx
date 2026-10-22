// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { useCloudSyncQueryRefresh } from "../../src/features/cloud-sync/useCloudSyncQueryRefresh";
import { useDesktopWorkspace } from "../../../../packages/app-shell/src/components/useDesktopWorkspace";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  subscribeCloudSyncWorkspaceChanges: vi.fn(async () => () => {}),
  getWorkspaceState: vi.fn(), getWorkspaceLayout: vi.fn(() => new Promise(() => {})),
  listWorkspaceEnvironments: vi.fn(async () => []), listDatabaseConnections: vi.fn(async () => []),
}));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("updates the workspace switcher for a different workspace without reloading current workspace resources", async () => {
  const workspace: commands.Workspace = {
    id: "ws", name: "Current", environmentType: "dev", mcpPolicy: "auto", isDefault: true,
    lastOpenedAt: null, createdAt: "now", updatedAt: "now", deletedAt: null, revision: 1,
  };
  vi.mocked(commands.getWorkspaceState).mockResolvedValue({ activeWorkspaceId: "ws", workspaces: [workspace] });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result } = renderHook(() => {
    useCloudSyncQueryRefresh();
    return useDesktopWorkspace();
  }, { wrapper });
  await waitFor(() => expect(result.current.activeWorkspace?.name).toBe("Current"));
  await waitFor(() => expect(client.getQueryState(["database-connections", "ws"])?.status).toBe("success"));
  const connectionReads = vi.mocked(commands.listDatabaseConnections).mock.calls.length;
  vi.mocked(commands.getWorkspaceState).mockResolvedValue({ activeWorkspaceId: "ws", workspaces: [workspace, { ...workspace, id: "other", name: "Downloaded workspace", isDefault: false }] });
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0]("other"));
  await waitFor(() => expect(result.current.workspaceQuery.data?.workspaces.map((row) => row.name)).toEqual(["Current", "Downloaded workspace"]));
  expect(commands.listDatabaseConnections).toHaveBeenCalledTimes(connectionReads);
  vi.mocked(commands.getWorkspaceState).mockResolvedValue({ activeWorkspaceId: "ws", workspaces: [workspace] });
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0]("other"));
  await waitFor(() => expect(result.current.workspaceQuery.data?.workspaces).toHaveLength(1));
});
