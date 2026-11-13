import { type Dispatch, type SetStateAction, useEffect, useRef, useState } from "react";
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

export function useDatabaseConnectionMutations({
  databaseTabs,
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
  const saveScope = useRef<string | null>(workspaceId);
  useEffect(() => {
    saveScope.current = workspaceId;
    return () => { saveScope.current = null; };
  }, [workspaceId]);
  const saveMutation = useMutation({
    mutationFn: async ({ input, secret }: { input: DatabaseConnectionInput; secret: string }) => {
      // Selecting a live transport for an existing definition also works when its
      // persistent jump has gone away. No business write is needed in that case.
      if (input.expectedSnapshot && onlyTemporaryRouteChanged(input, secret)) {
        bindTemporaryHandle("database", input.workspaceId, input.expectedSnapshot.id, input.temporaryHandle);
        return input.expectedSnapshot;
      }
      return saveDatabaseConnection(input, secret || null);
    },
    onMutate: ({ input }) => ({ previous: connectionTemporaryHandle("database", input.workspaceId, input.id) }),
    onError: (_error, { input }) => { void queryClient.invalidateQueries({ queryKey: databaseConnectionsQueryKey(input.workspaceId) }); },
    onSuccess: async (connection, { input }, before) => {
      const queryKey = databaseConnectionsQueryKey(connection.workspaceId);
      await queryClient.cancelQueries({ queryKey, exact: true });
      queryClient.setQueryData<DatabaseConnection[]>(queryKey, (current) =>
        replaceDatabaseConnectionInCache(current, connection),
      );
      void queryClient.invalidateQueries({ queryKey });
      if (saveScope.current !== connection.workspaceId) return;
      if (input.id && (before?.previous !== connectionTemporaryHandle("database", connection.workspaceId, connection.id)
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
      if (saveScope.current !== connection.workspaceId || version !== databaseRouteVersion(connection.workspaceId, connection.id)) return;
      const connectionId = connection.id;
      setTestResult(null);
      setConnectionState(connectionId, {
        message: formatDatabaseError(error, t),
        status: "failed",
      });
    },
    onSuccess: (result, connection, version) => {
      if (saveScope.current !== connection.workspaceId || version !== databaseRouteVersion(connection.workspaceId, connection.id)) return;
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
  const testInputMutation = useMutation({
    mutationFn: ({ input, secret }: { input: DatabaseConnectionInput; secret: string | null }) =>
      testDatabaseConnectionInput(input, secret),
    onSuccess: (result) => setTestResult(result),
    onError: (error) =>
      setTestResult({ ok: false, message: formatDatabaseError(error, t), serverVersion: null }),
  });

  return {
    deleteConfirm,
    deleteMutation,
    duplicateMutation,
    saveMutation,
    setDeleteConfirm,
    testInputMutation,
    testMutation,
  };
}
