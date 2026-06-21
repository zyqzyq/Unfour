// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { SshConnection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  listSshConnections: vi.fn(),
}));

import { listSshConnections } from "@unfour/command-client";
import { useSshConnections } from "./useSshConnections";

const listMock = vi.mocked(listSshConnections);

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("useSshConnections", () => {
  it("loads connections for the workspace", async () => {
    listMock.mockResolvedValue([{ id: "conn-1" } as SshConnection]);

    const { result } = renderHook(() => useSshConnections("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.data).toHaveLength(1));
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("stays disabled while the workspace id is empty", () => {
    renderHook(() => useSshConnections(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });
});
