import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  listSshConnections,
  listSshSessions,
} from "@unfour/command-client";
import { preloadSshWorkspace } from "./sshWorkspaceQueries";

vi.mock("@unfour/command-client", () => ({
  listSshConnections: vi.fn(),
  listSshSessions: vi.fn(),
}));

const listConnectionsMock = vi.mocked(listSshConnections);
const listSessionsMock = vi.mocked(listSshSessions);

beforeEach(() => {
  listConnectionsMock.mockReset().mockResolvedValue([]);
  listSessionsMock.mockReset().mockResolvedValue([]);
});

describe("preloadSshWorkspace", () => {
  it("prefetches SSH connections and sessions into the shared query cache", async () => {
    const queryClient = new QueryClient();

    await preloadSshWorkspace(queryClient, "workspace-one");

    expect(listConnectionsMock).toHaveBeenCalledWith("workspace-one");
    expect(listSessionsMock).toHaveBeenCalledWith("workspace-one");
    expect(queryClient.getQueryData(["ssh-connections", "workspace-one"])).toEqual([]);
    expect(queryClient.getQueryData(["ssh-sessions", "workspace-one"])).toEqual([]);
  });

  it("reuses fresh prefetched data during repeated navigation intent", async () => {
    const queryClient = new QueryClient();

    await preloadSshWorkspace(queryClient, "workspace-one");
    await preloadSshWorkspace(queryClient, "workspace-one");

    expect(listConnectionsMock).toHaveBeenCalledTimes(1);
    expect(listSessionsMock).toHaveBeenCalledTimes(1);
  });
});
