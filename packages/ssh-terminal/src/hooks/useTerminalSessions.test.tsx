// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { SshSessionSummary } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  listSshSessions: vi.fn(),
}));

import { listSshSessions } from "@unfour/command-client";
import { useTerminalSessions } from "./useTerminalSessions";

const listMock = vi.mocked(listSshSessions);

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

describe("useTerminalSessions", () => {
  it("loads sessions for the workspace", async () => {
    listMock.mockResolvedValue([{ sessionId: "s1" } as SshSessionSummary]);

    const { result } = renderHook(() => useTerminalSessions("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.data).toHaveLength(1));
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("stays disabled while the workspace id is empty", () => {
    renderHook(() => useTerminalSessions(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });
});
