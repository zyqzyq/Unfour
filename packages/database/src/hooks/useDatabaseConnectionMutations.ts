import { type Dispatch, type SetStateAction, useState } from "react";
import { useMutation, type QueryClient } from "@tanstack/react-query";
import {
  createCredential,
  deleteDatabaseConnection,
  rotateCredential,
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
import { formatDatabaseError } from "../result-utils";

export function useDatabaseConnectionMutations({
  databaseTabs,
  queryClient,
  removeConnection,
  selectedConnectionId,
  setConnectionState,
  setEditorOpen,
  setPassword,
  setSelectedDatabaseConnection,
  setSelectedTable,
  setTestResult,
  t,
  workspaceId,
}: {
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  queryClient: QueryClient;
  removeConnection: (connectionId: string) => void;
  selectedConnectionId: string | null;
  setConnectionState: (
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
  setEditorOpen: Dispatch<SetStateAction<boolean>>;
  setPassword: Dispatch<SetStateAction<string>>;
  setSelectedDatabaseConnection: (connectionId: string | null) => void;
  setSelectedTable: Dispatch<SetStateAction<DatabaseTable | null>>;
  setTestResult: Dispatch<SetStateAction<DatabaseTestResult | null>>;
  t: ReturnType<typeof useI18n>["t"];
  workspaceId: string;
}) {
  const saveMutation = useMutation({
    mutationFn: async ({ input, secret }: { input: DatabaseConnectionInput; secret: string }) => {
      let credentialRef = input.credentialRef ?? null;
      // Non-SQLite drivers persist the password through SecretStore and store
      // only the returned reference. An empty secret while editing keeps the
      // existing credential untouched.
      if (input.driver !== "sqlite" && secret.trim()) {
        if (credentialRef) {
          await rotateCredential({ workspaceId, credentialRef, secret });
        } else {
          const metadata = await createCredential({
            workspaceId,
            kind: "database",
            label: input.name,
            secret,
          });
          credentialRef = metadata.credentialRef;
        }
      }
      return saveDatabaseConnection({ ...input, credentialRef });
    },
    onSuccess: (connection) => {
      setPassword("");
      setSelectedDatabaseConnection(connection.id);
      setEditorOpen(false);
      setConnectionState(connection.id, {
        message: t("database.connection.savedBrowseSchema"),
        status: "disconnected",
      });
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
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
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  // Clone a connection into a new record. The stored credential is shared by
  // reusing its reference (the plaintext secret is never exposed to the client),
  // so the copy can connect immediately without re-entering the password.
  const duplicateMutation = useMutation({
    mutationFn: (connection: DatabaseConnection) =>
      saveDatabaseConnection({
        workspaceId,
        name: t("database.tree.duplicateName", { name: connection.name }),
        driver: connection.driver,
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
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const testMutation = useMutation({
    mutationFn: (connectionId: string) => testDatabaseConnection(workspaceId, connectionId),
    onMutate: (connectionId) => {
      setConnectionState(connectionId, {
        message: t("common.actions.connecting"),
        status: "connecting",
      });
    },
    onError: (error, connectionId) => {
      setTestResult(null);
      setConnectionState(connectionId, {
        message: formatDatabaseError(error),
        status: "failed",
      });
    },
    onSuccess: (result, connectionId) => {
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
      setTestResult({ ok: false, message: formatDatabaseError(error), serverVersion: null }),
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
