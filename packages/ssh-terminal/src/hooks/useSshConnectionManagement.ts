import { useEffect, useMemo, useState, type FormEvent, type RefObject } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getSshHostFingerprint,
  resetSshHostFingerprint,
  saveSshConnection,
  testSshConnection,
  type SshConnection,
  type SshConnectionInput,
  type SshHostFingerprintInfo,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import {
  defaultSshConnectionInput,
  sshConnectionToInput,
  sshConnectionContent,
} from "../model/ssh-connection-state";
import { formatTerminalError } from "../model/errors";

/** Owns connection drafts and trust dialogs; session execution stays with orchestration. */
export function useSshConnectionManagement({
  connections,
  connectMutation,
  workspaceId,
  workspaceScope,
}: {
  connections: SshConnection[];
  connectMutation: { error: unknown; reset: () => void; mutate: (connectionId: string) => void };
  workspaceId: string;
  workspaceScope: RefObject<string | null>;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const { selectedSshConnectionId: selectedConnectionId, setSelectedSshConnection } = useWorkspaceStore();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"new" | "edit" | null>(null);
  const [trustDialogState, setTrustDialogState] = useState<{
    open: boolean;
    connectionId: string | null;
    host: string;
    port: number;
    fingerprint: SshHostFingerprintInfo | null | undefined;
    mismatchError: string | null;
  }>({
    open: false,
    connectionId: null,
    host: "",
    port: 22,
    fingerprint: undefined,
    mismatchError: null,
  });
  const [form, setForm] = useState<SshConnectionInput>(() =>
    defaultSshConnectionInput(workspaceId),
  );
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(
    null,
  );

  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  const [previousSelectedConnectionId, setPreviousSelectedConnectionId] = useState(selectedConnectionId);

  useEffect(() => {
    if (!connections.length) {
      if (selectedConnectionId) {
        setSelectedSshConnection(null);
      }
      return;
    }

    if (dialogMode === "new" || (dialogOpen && dialogMode === "edit")) {
      return;
    }

    if (!selectedConnectionId || !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedSshConnection(connections[0].id);
    }
  }, [connections, selectedConnectionId, setSelectedSshConnection, dialogMode, dialogOpen]);

  const latestFormConnection = connections.find((item) => item.id === form.id) ?? null;
  const formBaseline = form.expectedSnapshot;
  const externalChange = Boolean(formBaseline && sshConnectionContent(formBaseline) !== sshConnectionContent(latestFormConnection));
  const formDirty = Boolean(formBaseline && JSON.stringify(form) !== JSON.stringify(sshConnectionToInput(formBaseline, workspaceId)));
  if (externalChange && !formDirty && latestFormConnection) {
    setForm(sshConnectionToInput(latestFormConnection, workspaceId));
  }

  // Sync form state when the selected connection changes (render-time adjustment pattern).
  if (selectedConnectionId !== previousSelectedConnectionId) {
    setPreviousSelectedConnectionId(selectedConnectionId);
    if (selectedConnection && dialogMode !== "new") {
      setForm(sshConnectionToInput(selectedConnection, workspaceId));
    }
  }

  const saveMutation = useMutation({
    mutationFn: saveSshConnection,
    onError: (_error, input) => { void queryClient.invalidateQueries({ queryKey: ["ssh-connections", input.workspaceId] }); },
    onSuccess: async (connection) => {
      const queryKey = ["ssh-connections", connection.workspaceId];
      await queryClient.cancelQueries({ queryKey, exact: true });
      queryClient.setQueryData<SshConnection[]>(queryKey, (current = []) =>
        [...current.filter((item) => item.id !== connection.id), connection]);
      void queryClient.invalidateQueries({ queryKey });
      if (workspaceScope.current !== connection.workspaceId) return;
      setSelectedSshConnection(connection.id);
      setDialogOpen(false);
      setDialogMode(null);
    },
  });

  // Test a connection using the dedicated test endpoint, which accepts the full
  // form payload. The backend resolves stored credentials via `credential_ref`
  // (carried on the form for saved connections) and applies `secret` as an
  // override, so a newly typed password/passphrase is honored as well. The
  // endpoint spins up and tears down its own throwaway session.
  const testMutation = useMutation({
    mutationFn: async ({
      form,
    }: {
      form: SshConnectionInput;
    }) => {
      return testSshConnection({ ...form, workspaceId });
    },
    onSuccess: (result) =>
      setTestResult({ ok: result.ok, message: result.message }),
    onError: (error) =>
      setTestResult({ ok: false, message: formatTerminalError(error, t) }),
  });

  // Reset a previously trusted host key (e.g. after a mismatch) and then
  // re-attempt the connection. Surfaces failures via the feedback toast.
  const resetHostKeyMutation = useMutation({
    mutationFn: (input: { host: string; port: number }) =>
      resetSshHostFingerprint({ workspaceId, ...input }),
    onSuccess: (result) => {
      if (result.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
        feedback.info(t("feedback.activityRecordFailed"));
      }
      setTrustDialogState((prev) => ({ ...prev, open: false, mismatchError: null }));
      const targetConnectionId = trustDialogState.connectionId ?? selectedConnectionId;
      if (targetConnectionId) {
        connectMutation.reset();
        connectMutation.mutate(targetConnectionId);
      }
    },
    onError: (error) => handleError(error, { key: "ssh.trust.resetFailed" }),
  });

  function resetHostAndReconnect() {
    if (!trustDialogState.host) {
      return;
    }
    resetHostKeyMutation.mutate({
      host: trustDialogState.host,
      port: trustDialogState.port,
    });
  }

  function updateForm(patch: Partial<SshConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function newConnection() {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    setSelectedSshConnection(null);
    setForm(defaultSshConnectionInput(workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogMode("new");
    setDialogOpen(true);
  }

  function openConnectionSettings(connection?: SshConnection | null) {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    const target = connection ?? selectedConnection;
    if (target) {
      setSelectedSshConnection(target.id);
      setForm(sshConnectionToInput(target, workspaceId));
      setDialogMode("edit");
    } else {
      setForm(defaultSshConnectionInput(workspaceId));
      setDialogMode("new");
    }
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

  function editConnection(connection: SshConnection) {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    setSelectedSshConnection(connection.id);
    setForm(sshConnectionToInput(connection, workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogMode("edit");
    setDialogOpen(true);
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate({
      ...form,
      workspaceId,
      credentialRef: form.credentialRef?.trim() || null,
      keyPath: form.keyPath?.trim() || null,
      // Preserve the secret verbatim (passwords may contain spaces); an empty
      // field means "keep the saved password".
      secret: form.secret ? form.secret : null,
    });
  }

  function testConnection() {
    testMutation.reset();
    setTestResult(null);
    testMutation.mutate({ form });
  }

  function connectSelectedConnection() {
    connectMutation.reset();
    if (!selectedConnectionId || !selectedConnection) {
      newConnection();
      return;
    }

    const host = selectedConnection.host;
    const port = selectedConnection.port ?? 22;

    // Check if we already trust this host.
    getSshHostFingerprint({ workspaceId, host, port })
      .then((info) => {
        if (info) {
          // Already trusted — connect directly.
          connectMutation.mutate(selectedConnectionId);
        } else {
          // First trust — show confirmation dialog.
          setTrustDialogState({
            open: true,
            connectionId: selectedConnectionId,
            host,
            port,
            fingerprint: null,
            mismatchError: null,
          });
        }
      })
      .catch(() => {
        // If fingerprint check fails, proceed with connection anyway
        // (the backend TOFU will handle it).
        connectMutation.mutate(selectedConnectionId);
      });
  }

  function confirmTrustAndConnect() {
    if (!trustDialogState.connectionId) return;
    connectMutation.reset();
    connectMutation.mutate(trustDialogState.connectionId);
  }

  // Detect host-key mismatch errors from connect failures.
  useEffect(() => {
    const error = connectMutation.error;
    if (!error) return;
    const message = error instanceof Error ? error.message : String(error);
    if (
      message.includes("host key verification failed") ||
      message.includes("fingerprint does not match")
    ) {
      const conn = selectedConnection;
      // eslint-disable-next-line react-hooks/set-state-in-effect -- surfacing mutation error as trust dialog is an external-system sync
      setTrustDialogState({
        open: true,
        connectionId: null,
        host: conn?.host ?? "",
        port: conn?.port ?? 22,
        fingerprint: undefined,
        mismatchError: message,
      });
    }
  }, [connectMutation.error, selectedConnection]);

  function reloadForm() {
    if (latestFormConnection) setForm(sshConnectionToInput(latestFormConnection, workspaceId));
    else { setDialogOpen(false); setDialogMode(null); }
    saveMutation.reset();
  }

  function changeDialogOpen(open: boolean) {
    setDialogOpen(open);
    if (!open) {
      setDialogMode(null);
      setTestResult(null);
    }
  }

  const canTest =
    Boolean(form.host?.trim()) &&
    Boolean(form.username?.trim()) &&
    (form.authKind !== "private-key" || Boolean(form.keyPath?.trim())) &&
    (form.authKind !== "password" || Boolean(form.id) || Boolean(form.secret?.trim()));

  return {
    canTest,
    changeDialogOpen,
    confirmTrustAndConnect,
    connectSelectedConnection,
    dialogOpen,
    editConnection,
    externalChange,
    form,
    newConnection,
    openConnectionSettings,
    reloadForm,
    resetHostAndReconnect,
    resetHostKeyMutation,
    saveMutation,
    selectedConnection,
    setTestResult,
    setTrustDialogState,
    submitConnection,
    testConnection,
    testMutation,
    testResult,
    trustDialogState,
    updateForm,
  };
}
