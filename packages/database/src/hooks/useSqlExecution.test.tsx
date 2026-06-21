// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  executeDatabaseQuery: vi.fn(),
}));

import { executeDatabaseQuery } from "@unfour/command-client";
import { useSqlExecution } from "./useSqlExecution";

const executeMock = vi.mocked(executeDatabaseQuery);

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("useSqlExecution", () => {
  it("executes the query and clears the confirmation flag on success", async () => {
    const queryResult = { rows: [] } as unknown as DatabaseQueryResult;
    executeMock.mockResolvedValue(queryResult);
    const callbacks = {
      onConfirmationRequired: vi.fn(),
      onExecuteStart: vi.fn(),
      onSuccess: vi.fn(),
    };

    const { result } = renderHook(
      () =>
        useSqlExecution({
          connectionId: "conn-1",
          sql: "select 1",
          workspaceId: "ws-1",
          ...callbacks,
        }),
      { wrapper: createWrapper() },
    );

    result.current.mutate(false);

    await waitFor(() => expect(callbacks.onSuccess).toHaveBeenCalled());
    expect(callbacks.onExecuteStart).toHaveBeenCalled();
    expect(executeMock).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      connectionId: "conn-1",
      sql: "select 1",
      limit: 100,
      confirmMutation: false,
    });
    expect(callbacks.onConfirmationRequired).toHaveBeenLastCalledWith(false);
    expect(callbacks.onSuccess).toHaveBeenCalledWith(queryResult, false);
  });

  it("requests confirmation when the backend reports CONFIRMATION_REQUIRED", async () => {
    executeMock.mockRejectedValue({ code: "CONFIRMATION_REQUIRED" });
    const onConfirmationRequired = vi.fn();
    const onError = vi.fn();

    const { result } = renderHook(
      () =>
        useSqlExecution({
          connectionId: "conn-1",
          onConfirmationRequired,
          onError,
          onExecuteStart: vi.fn(),
          onSuccess: vi.fn(),
          sql: "delete from t",
          workspaceId: "ws-1",
        }),
      { wrapper: createWrapper() },
    );

    result.current.mutate(false);

    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(onConfirmationRequired).toHaveBeenCalledWith(true);
  });

  it("does not flag confirmation for ordinary errors", async () => {
    executeMock.mockRejectedValue(new Error("syntax error"));
    const onConfirmationRequired = vi.fn();

    const { result } = renderHook(
      () =>
        useSqlExecution({
          connectionId: "conn-1",
          onConfirmationRequired,
          onExecuteStart: vi.fn(),
          onSuccess: vi.fn(),
          sql: "selct 1",
          workspaceId: "ws-1",
        }),
      { wrapper: createWrapper() },
    );

    result.current.mutate(true);

    await waitFor(() => expect(onConfirmationRequired).toHaveBeenCalled());
    expect(onConfirmationRequired).toHaveBeenCalledWith(false);
  });
});
