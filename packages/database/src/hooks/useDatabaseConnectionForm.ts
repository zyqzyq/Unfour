import { useState } from "react";
import type { DatabaseConnection, DatabaseConnectionInput, DatabaseTestResult } from "@unfour/command-client";
import {
  databaseConnectionToInput,
  emptyDatabaseConnectionForm,
} from "../model/database-credentials";

/** Selection changes discard credentials and drafts; query/cache renders do not. */
export function useDatabaseConnectionForm(
  workspaceId: string,
  selectedConnectionId: string | null,
  selectedConnection: DatabaseConnection | null,
) {
  const [selection, setSelection] = useState({ workspaceId, selectedConnectionId });
  const [editorOpen, setEditorOpen] = useState(false);
  const [testResult, setTestResult] = useState<DatabaseTestResult | null>(null);
  const [password, setPassword] = useState("");
  const [form, setForm] = useState<DatabaseConnectionInput>(() => emptyDatabaseConnectionForm(workspaceId));

  if (workspaceId !== selection.workspaceId) {
    setSelection({ workspaceId, selectedConnectionId });
    setPassword("");
    setForm(emptyDatabaseConnectionForm(workspaceId));
    setEditorOpen(false);
    setTestResult(null);
  } else if (selectedConnectionId !== selection.selectedConnectionId) {
    setSelection({ workspaceId, selectedConnectionId });
    setPassword("");
    if (selectedConnection) {
      setForm(databaseConnectionToInput(selectedConnection, workspaceId));
      setTestResult(null);
    }
  }

  function hydrateFormFromConnection(connection: DatabaseConnection) {
    setPassword("");
    setForm(databaseConnectionToInput(connection, workspaceId));
  }

  return {
    editorOpen,
    setEditorOpen,
    testResult,
    setTestResult,
    password,
    setPassword,
    form,
    setForm,
    hydrateFormFromConnection,
  };
}
