import { type Dispatch, type SetStateAction, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useMutation, type QueryClient } from "@tanstack/react-query";
import {
  bindTemporaryHandle,
  connectionTemporaryHandle,
  databaseActionContext,
  databaseRouteVersion,
  deleteDatabaseConnection,
  saveDatabaseConnection,
  testDatabaseConnection,
  testDatabaseConnectionInput,
} from "@unfour/command-client";
import type {
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseTable,
  DatabaseTestResult,
} from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { useDatabaseTabs } from "./useDatabaseTabs";
import type { DatabaseConnectionSessionState } from "../model/types";
import {
  databaseConnectionsQueryKey,
  replaceDatabaseConnectionInCache,
} from "./useDatabaseConnections";
import { formatDatabaseError } from "../result-utils";
import { onlyTemporaryRouteChanged } from "../model/database-credentials";

type EditorScope = { workspaceId: string; connectionId?: string };
type EditorAction = "save" | "test";
type EditorRequest = { scope: EditorScope; action: EditorAction };
type SaveInput = { input: DatabaseConnectionInput; secret: string };
type TestInput = { input: DatabaseConnectionInput; secret: string | null };

export function useDatabaseConnectionMutations({
  databaseTabs,
  editorOpen,
  form,
  hydrateFormFromConnection,
  queryClient,
  removeConnection,
  onRouteChanged,
  selectedConnectionId,
  setConnectionState,
  setEditorOpen,
  setSelectedDatabaseConnection,
  setSelectedTable,
  setTestResult,
  t,
  workspaceId,
}: {
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  editorOpen: boolean;
  form: DatabaseConnectionInput;
  hydrateFormFromConnection: (connection: DatabaseConnection) => void;
  queryClient: QueryClient;
  removeConnection: (connectionId: string) => void;
  onRouteChanged?: (connectionId: string) => void;
  selectedConnectionId: string | null;
  setConnectionState: (
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
  setEditorOpen: Dispatch<SetStateAction<boolean>>;
  setSelectedDatabaseConnection: (connectionId: string | null) => void;
  setSelectedTable: Dispatch<SetStateAction<DatabaseTable | null>>;
  setTestResult: Dispatch<SetStateAction<DatabaseTestResult | null>>;
  t: ReturnType<typeof useI18n>["t"];
  workspaceId: string;
}) {
  const workspaceScope = useRef<{ workspaceId: string } | null>(null);
  useLayoutEffect(() => {
    workspaceScope.current = { workspaceId };
    return () => { workspaceScope.current = null; };
  }, [workspaceId]);
  const connectionId = form.id;
  const scope = useMemo(() => editorOpen ? { workspaceId, connectionId } : null,
    [connectionId, editorOpen, workspaceId]);
  const editorScope = useRef<EditorScope | null>(null);
  const inFlight = useRef<EditorRequest | null>(null);
  const [pendingRequest, setPendingRequest] = useState<EditorRequest | null>(null);
  const pending = pendingRequest?.scope === scope ? pendingRequest.action : null;
  const currentForm = useRef(form);
  useLayoutEffect(() => { currentForm.current = form; }, [form]);

  const saveMutation = useMutation({
    mutationFn: async ({ input, secret }: SaveInput & { scope: EditorScope }) => {
      // Selecting a live transport for an existing definition also works when its
      // persistent jump has gone away. No business write is needed in that case.
      if (input.expectedSnapshot && onlyTemporaryRouteChanged(input, secret)) {
        bindTemporaryHandle("database", input.workspaceId, input.expectedSnapshot.id, input.temporaryHandle);
        return input.expectedSnapshot;
      }
      return saveDatabaseConnection(input, secret || null);
    },
    onMutate: ({ input }) => connectionTemporaryHandle("database", input.workspaceId, input.id),
    onError: (_error, { input }) => { void queryClient.invalidateQueries({ queryKey: databaseConnectionsQueryKey(input.workspaceId) }); },
    onSuccess: async (connection, { input, scope }, previous) => {
      const queryKey = databaseConnectionsQueryKey(connection.workspaceId);
      await queryClient.cancelQueries({ queryKey, exact: true });
      queryClient.setQueryData<DatabaseConnection[]>(queryKey, (current) =>
        replaceDatabaseConnectionInCache(current, connection),
      );
      void queryClient.invalidateQueries({ queryKey });
      if (scope.workspaceId !== connection.workspaceId || editorScope.current !== scope) return;
      if (input.id && (previous !== connectionTemporaryHandle("database", connection.workspaceId, connection.id)
        || input.expectedSnapshot?.sshConnectionId !== connection.sshConnectionId
        || input.expectedSnapshot?.sshRevision !== connection.sshRevision)) {
        onRouteChanged?.(connection.id);
      }
      hydrateFormFromConnection(connection);
      setSelectedDatabaseConnection(connection.id);
      setEditorOpen(false);
      setConnectionState(connection.id, {
        message: t("database.connection.savedBrowseSchema"),
        status: "disconnected",
      });
    },
  });

  const [deleteConfirm, setDeleteConfirm] = useState<DatabaseConnection | null>(null);
  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteDatabaseConnection(workspaceId, connectionId),
    onSuccess: (_result, connectionId) => {
      removeConnection(connectionId);
      // Only reset the active workspace when the deleted connection was the one
      // in use; deleting another connection from the context menu must not clear
      // the current query or table view.
      if (connectionId === selectedConnectionId) {
        setSelectedDatabaseConnection(null);
        setTestResult(null);
        setSelectedTable(null);
      }
      databaseTabs.removeConnectionTabs(connectionId);
      setDeleteConfirm(null);
      queryClient.invalidateQueries({ queryKey: databaseConnectionsQueryKey(workspaceId) });
    },
  });

  // Clone a connection into a new record. The stored credential is shared by
  // reusing its reference (the plaintext secret is never exposed to the client),
  // so the copy can connect immediately without re-entering the password.
  // Password replacement publishes a fresh reference for the edited copy.
  const duplicateMutation = useMutation({
    mutationFn: (connection: DatabaseConnection) =>
      saveDatabaseConnection({
        workspaceId,
        name: t("database.tree.duplicateName", { name: connection.name }),
        driver: connection.driver,
        sshConnectionId: connection.sshConnectionId,
        expectedSshRevision: connection.sshRevision,
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        sslMode: connection.sslMode,
        sqlitePath: connection.sqlitePath,
        credentialRef: connection.credentialRef,
        readOnly: connection.readOnly,
      }),
    onSuccess: (created) => {
      setSelectedDatabaseConnection(created.id);
      queryClient.invalidateQueries({ queryKey: databaseConnectionsQueryKey(workspaceId) });
    },
  });

  const testMutation = useMutation({
    mutationFn: (connection: DatabaseConnection) => testDatabaseConnection(connection.workspaceId, connection.id, databaseActionContext(connection.workspaceId, connection.id, connection)),
    onMutate: (connection) => {
      setConnectionState(connection.id, {
        message: t("common.actions.connecting"),
        status: "connecting",
      });
      return databaseRouteVersion(connection.workspaceId, connection.id);
    },
    onError: (error, connection, version) => {
      if (workspaceScope.current?.workspaceId !== connection.workspaceId || version !== databaseRouteVersion(connection.workspaceId, connection.id)) return;
      const connectionId = connection.id;
      setTestResult(null);
      setConnectionState(connectionId, {
        message: formatDatabaseError(error, t),
        status: "failed",
      });
    },
    onSuccess: (result, connection, version) => {
      if (workspaceScope.current?.workspaceId !== connection.workspaceId || version !== databaseRouteVersion(connection.workspaceId, connection.id)) return;
      const connectionId = connection.id;
      setTestResult(result);
      setConnectionState(connectionId, {
        message: result.message,
        serverVersion: result.serverVersion,
        status: result.ok ? "connected" : "failed",
      });
      if (result.ok) {
        queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, connectionId] });
      }
    },
  });

  // Validate a connection from the dialog form without persisting it. Used by
  // the "Test connection" button, which must work for brand-new (unsaved)
  // connections. Unlike `testMutation` (by saved id, which also opens a
  // session), this only checks connectivity and leaves state disconnected.
  const testInput = useMutation({
    mutationKey: ["database-test-input", workspaceId],
    mutationFn: ({ input, secret }: TestInput & { scope: EditorScope }) =>
      testDatabaseConnectionInput(input, secret),
    onSuccess: (result, { input, scope }) => {
      if (editorScope.current === scope && currentForm.current === input) setTestResult(result);
    },
    onError: (error, { input, scope }) => {
      if (editorScope.current === scope && currentForm.current === input) {
        setTestResult({ ok: false, message: formatDatabaseError(error, t), serverVersion: null });
      }
    },
  });

  const { reset: resetSave } = saveMutation;
  const { reset: resetTest } = testInput;
  useLayoutEffect(() => {
    resetSave();
    resetTest();
    editorScope.current = scope;
    return () => { editorScope.current = null; };
  }, [resetSave, resetTest, scope]);

  async function runEditorMutation<Result>(action: EditorAction, run: (scope: EditorScope) => Promise<Result>) {
    const scope = editorScope.current;
    if (!scope || inFlight.current?.scope === scope) return;
    // Acquire the lock before React commits the disabled controls.
    const request = { scope, action };
    inFlight.current = request;
    setPendingRequest(request);
    try {
      return await run(scope);
    } finally {
      if (inFlight.current === request) inFlight.current = null;
      setPendingRequest((current) => current === request ? null : current);
    }
  }

  return {
    deleteConfirm,
    deleteMutation,
    duplicateMutation,
    saveMutation: {
      error: saveMutation.error,
      isPending: pending === "save",
      reset: resetSave,
      mutateAsync: (variables: SaveInput) => runEditorMutation("save", (scope) => saveMutation.mutateAsync({ ...variables, scope })),
    },
    setDeleteConfirm,
    testInputMutation: {
      isPending: pending === "test",
      reset: resetTest,
      mutateAsync: (variables: TestInput) => runEditorMutation("test", (scope) => testInput.mutateAsync({ ...variables, scope })),
    },
    testMutation,
  };
}
