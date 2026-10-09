// @vitest-environment jsdom
import type { ReactNode } from "react";
import { useState } from "react";
import type { DatabaseConnection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { useDatabaseTabs } from "./useDatabaseTabs";

vi.mock("@unfour/command-client", () => ({
  createCredential: vi.fn(),
  deleteDatabaseConnection: vi.fn(),
  listDatabaseConnections: vi.fn(),
  rotateCredential: vi.fn(),
  saveDatabaseConnection: vi.fn(),
  testDatabaseConnection: vi.fn(),
  testDatabaseConnectionInput: vi.fn(),
}));

import {
  createCredential,
  listDatabaseConnections,
  rotateCredential,
  saveDatabaseConnection,
} from "@unfour/command-client";
import { DATABASE_PASSWORD_KIND, databaseConnectionToInput } from "../model/database-credentials";
import { useDatabaseConnectionForm } from "./useDatabaseConnectionForm";
import { useDatabaseConnectionMutations } from "./useDatabaseConnectionMutations";
import {
  databaseConnectionsQueryKey,
  useDatabaseConnections,
} from "./useDatabaseConnections";

const createMock = vi.mocked(createCredential);
const listMock = vi.mocked(listDatabaseConnections);
const rotateMock = vi.mocked(rotateCredential);
const saveMock = vi.mocked(saveDatabaseConnection);

const OLD_REF = "unfour:ws-other:database-password:old-cred";
const CURRENT_REF = "unfour:ws-current:database-password:cred-1";
const NEW_REF = "unfour:ws-current:database-password:new-cred";

function persisted(overrides: Partial<DatabaseConnection> = {}): DatabaseConnection {
  return {
    id: "db-a",
    workspaceId: "ws-current",
    name: "App DB",
    driver: "postgres",
    host: "localhost",
    port: 5432,
    database: "app",
    username: "dev",
    sslMode: null,
    sqlitePath: null,
    credentialRef: OLD_REF,
    readOnly: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
    ...overrides,
  };
}

function createWrapper(initial: DatabaseConnection) {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  client.setQueryData(databaseConnectionsQueryKey(initial.workspaceId), [initial]);
  return {
    client,
    Wrapper: function Wrapper({ children }: { children: ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
    },
  };
}

function useEditorSession(workspaceId: string, selectedConnectionId: string) {
  const queryClient = useQueryClient();
  const connectionsQuery = useDatabaseConnections(workspaceId);
  const connections = connectionsQuery.data ?? [];
  const [selectedId, setSelectedId] = useState<string | null>(selectedConnectionId);
  const selected = connections.find((item) => item.id === selectedId) ?? null;
  const formState = useDatabaseConnectionForm(workspaceId, selectedId, selected);
  const mutations = useDatabaseConnectionMutations({
    databaseTabs: {
      removeConnectionTabs: vi.fn(),
    } as unknown as ReturnType<typeof useDatabaseTabs>,
    hydrateFormFromConnection: formState.hydrateFormFromConnection,
    queryClient,
    removeConnection: vi.fn(),
    selectedConnectionId: selectedId,
    setConnectionState: vi.fn(),
    setEditorOpen: formState.setEditorOpen,
    setSelectedDatabaseConnection: setSelectedId,
    setSelectedTable: vi.fn(),
    setTestResult: formState.setTestResult,
    t: ((key: string) => key) as never,
    workspaceId,
  });
  return { connections, formState, mutations, queryClient, selectedId };
}

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockImplementation(() => new Promise(() => {}));
  createMock.mockResolvedValue({
    workspaceId: "ws-current",
    kind: DATABASE_PASSWORD_KIND,
    label: "App DB",
    credentialRef: NEW_REF,
  });
  saveMock.mockImplementation(async (input) =>
    persisted({
      id: input.id ?? "db-a",
      name: input.name,
      credentialRef: input.credentialRef ?? null,
      revision: 2,
      updatedAt: "2026-01-02T00:00:00Z",
    }),
  );
});

afterEach(() => vi.resetAllMocks());

describe("database credential save", () => {
  it("sends a replacement in the backend save and retains the returned reference", async () => {
    const existing = persisted({ credentialRef: CURRENT_REF });
    saveMock.mockResolvedValue(persisted({ credentialRef: NEW_REF }));
    const { Wrapper } = createWrapper(existing);
    const { result } = renderHook(() => useEditorSession("ws-current", existing.id), { wrapper: Wrapper });
    await act(async () => {
      await result.current.mutations.saveMutation.mutateAsync({ input: databaseConnectionToInput(existing, "ws-current"), secret: "replacement" });
    });
    expect(saveMock).toHaveBeenCalledWith(expect.objectContaining({ credentialRef: CURRENT_REF }), "replacement");
    expect(createMock).not.toHaveBeenCalled();
    expect(rotateMock).not.toHaveBeenCalled();
    expect(result.current.formState.form.credentialRef).toBe(NEW_REF);
    await act(async () => {
      await result.current.mutations.saveMutation.mutateAsync({ input: result.current.formState.form, secret: "" });
    });
    expect(saveMock).toHaveBeenLastCalledWith(expect.objectContaining({ credentialRef: NEW_REF }), null);
  });
  it("sends an explicit cleared reference without rotating credentials", async () => {
    const existing = persisted({ credentialRef: CURRENT_REF });
    const { Wrapper } = createWrapper(existing);
    const { result } = renderHook(() => useEditorSession("ws-current", existing.id), { wrapper: Wrapper });
    await act(async () => {
      await result.current.mutations.saveMutation.mutateAsync({ input: { ...databaseConnectionToInput(existing, "ws-current"), credentialRef: null }, secret: "" });
    });
    expect(saveMock).toHaveBeenCalledWith(expect.objectContaining({ credentialRef: null }), null);
    expect(rotateMock).not.toHaveBeenCalled();
  });
  it("keeps the unsaved replacement and dialog open on a backend failure", async () => {
    const existing = persisted({ credentialRef: CURRENT_REF });
    const { Wrapper } = createWrapper(existing);
    const { result } = renderHook(() => useEditorSession("ws-current", existing.id), { wrapper: Wrapper });
    act(() => { result.current.formState.setEditorOpen(true); result.current.formState.setPassword("replacement"); });
    saveMock.mockRejectedValue(new Error("save failed"));
    await act(async () => {
      await expect(result.current.mutations.saveMutation.mutateAsync({ input: databaseConnectionToInput(existing, "ws-current"), secret: "replacement" })).rejects.toThrow("save failed");
    });
    expect(result.current.formState.password).toBe("replacement");
    expect(result.current.formState.editorOpen).toBe(true);
    expect(rotateMock).not.toHaveBeenCalled();
  });
});
