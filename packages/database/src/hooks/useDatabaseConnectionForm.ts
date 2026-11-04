import { useState } from "react";
import type { DatabaseConnection, DatabaseConnectionInput, DatabaseTestResult } from "@unfour/command-client";
import {
  databaseConnectionToInput,
  databaseConnectionContent,
  emptyDatabaseConnectionForm,
} from "../model/database-credentials";

/** Refresh clean drafts; keep dirty drafts and their original save precondition. */
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
  const [observedConnection, setObservedConnection] = useState(selectedConnection);
  const baseline = form.expectedSnapshot;
  const latest = selectedConnection?.id === form.id ? selectedConnection : null;
  const externalChange = Boolean(baseline && databaseConnectionContent(baseline) !== databaseConnectionContent(latest));
  const dirty = Boolean(baseline && (password || JSON.stringify(form) !== JSON.stringify(databaseConnectionToInput(baseline, workspaceId))));

  if (workspaceId !== selection.workspaceId) {
    setSelection({ workspaceId, selectedConnectionId });
    setPassword("");
    setForm(emptyDatabaseConnectionForm(workspaceId));
    setEditorOpen(false);
    setTestResult(null);
  } else if (selectedConnectionId !== selection.selectedConnectionId && !(editorOpen && form.id && !selectedConnectionId)) {
    setSelection({ workspaceId, selectedConnectionId });
    setPassword("");
    if (selectedConnection) {
      setForm(databaseConnectionToInput(selectedConnection, workspaceId));
      setTestResult(null);
    }
  } else if (observedConnection !== selectedConnection) {
    setObservedConnection(selectedConnection);
    if (externalChange && !dirty && latest) {
      setForm(databaseConnectionToInput(latest, workspaceId));
      setTestResult(null);
    }
  }

  function hydrateFormFromConnection(connection: DatabaseConnection) {
    setPassword("");
    setForm(databaseConnectionToInput(connection, workspaceId));
  }

  return {
    externalChange,
    reloadLatest: () => {
      if (latest) hydrateFormFromConnection(latest);
      else setEditorOpen(false);
      setTestResult(null);
    },
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
