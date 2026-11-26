import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getSshHostFingerprint,
  resetSshHostFingerprint,
  saveSshConnection,
  testSshConnection,
  type SshConnection,
  type SshConnectionInput,
  type SshHostFingerprintInfo,
  type SshTestResult,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import {
  defaultSshConnectionInput,
  sshConnectionToInput,
  sshConnectionContent,
} from "../model/ssh-connection-state";
import { formatTerminalError } from "../model/errors";
import { sshConnectionsQueryKey } from "./sshWorkspaceQueries";

/** Owns connection drafts and trust dialogs; session execution stays with orchestration. */
export function useSshConnectionManagement({
  connections,
  connectMutation,
  workspaceId,
}: {
  connections: SshConnection[];
  connectMutation: { error: unknown; reset: () => void; mutate: (connectionId: string) => void };
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const { selectedSshConnectionId: selectedConnectionId, setSelectedSshConnection } = useWorkspaceStore();
  const [dialogMode, setDialogMode] = useState<"new" | "edit" | null>(null);
  const dialogOpen = dialogMode !== null;
  const editorBusy = useRef(false);
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
  const [tested, setTested] = useState<{ form: SshConnectionInput; result: SshTestResult } | null>(null);
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

    if (dialogOpen) {
      return;
    }

    if (!selectedConnectionId || !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedSshConnection(connections[0].id);
    }
  }, [connections, selectedConnectionId, setSelectedSshConnection, dialogOpen]);

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
    networkMode: "always",
    mutationFn: saveSshConnection,
    onError: (_error, input) => { void queryClient.invalidateQueries({ queryKey: sshConnectionsQueryKey(input.workspaceId) }); },
    onSuccess: async (connection) => {
      const queryKey = sshConnectionsQueryKey(connection.workspaceId);
      await queryClient.cancelQueries({ queryKey, exact: true });
      queryClient.setQueryData<SshConnection[]>(queryKey, (current = []) =>
        [...current.filter((item) => item.id !== connection.id), connection]);
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  // Test a connection using the dedicated test endpoint, which accepts the full
  // form payload. The backend resolves stored credentials via `credential_ref`
  // (carried on the form for saved connections) and applies `secret` as an
  // override, so a newly typed password/passphrase is honored as well. The
  // endpoint spins up and tears down its own throwaway session.
  const testMutation = useMutation({
    networkMode: "always",
    mutationFn: (input: SshConnectionInput) => testSshConnection(input),
  });
  const { reset: resetTest } = testMutation;
  const { reset: resetSave } = saveMutation;
  const resetEditorActions = useCallback(() => {
    editorBusy.current = false;
    setTested(null);
    resetTest();
    resetSave();
  }, [resetSave, resetTest]);
  // Detach per-call dialog callbacks on target changes and unmount. The save
  // mutation's cache update still runs for its original workspace.
  useLayoutEffect(() => resetEditorActions, [form.id, workspaceId, resetEditorActions]);

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
    if (editorBusy.current) return;
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function openEditor(target: SshConnection | null) {
    connectMutation.reset();
    resetEditorActions();
    if (target) setSelectedSshConnection(target.id);
    setForm(target ? sshConnectionToInput(target, workspaceId) : defaultSshConnectionInput(workspaceId));
    setDialogMode(target ? "edit" : "new");
    setTrustDialogState((prev) => ({ ...prev, open: false }));
  }

  function newConnection() {
    setSelectedSshConnection(null);
    openEditor(null);
  }

  function finishEditorAction() {
    editorBusy.current = false;
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    if (!dialogOpen || editorBusy.current || externalChange) return;
    editorBusy.current = true;
    saveMutation.mutate({
      ...form,
      workspaceId,
      credentialRef: form.credentialRef?.trim() || null,
      keyPath: form.keyPath?.trim() || null,
      // Preserve the secret verbatim (passwords may contain spaces); an empty
      // field means "keep the saved password".
      secret: form.secret ? form.secret : null,
    }, {
      onSuccess: (connection) => {
        setSelectedSshConnection(connection.id);
        setDialogMode(null);
      },
      onSettled: finishEditorAction,
    });
  }

  function testConnection() {
    if (!dialogOpen || editorBusy.current || !canTest) return;
    // Guard both actions synchronously, before the disabled fieldset is painted.
    editorBusy.current = true;
    testMutation.mutate(form, {
      onSuccess: (result) => setTested({ form, result }),
      onError: (error) => setTested({ form, result: { ok: false, message: formatTerminalError(error, t) } }),
      onSettled: finishEditorAction,
    });
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
    if (editorBusy.current) return;
    if (latestFormConnection) setForm(sshConnectionToInput(latestFormConnection, workspaceId));
    else setDialogMode(null);
    resetEditorActions();
  }

  function changeDialogOpen(open: boolean) {
    if (open || editorBusy.current) return;
    setDialogMode(null);
    resetEditorActions();
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
    editConnection: openEditor,
    externalChange,
    form,
    newConnection,
    openConnectionSettings: (target?: SshConnection | null) => openEditor(target ?? selectedConnection),
    reloadForm,
    resetHostAndReconnect,
    resetHostKeyMutation,
    saveMutation,
    selectedConnection,
    closeTestResult: () => setTested(null),
    setTrustDialogState,
    submitConnection,
    testConnection,
    testMutation,
    testResult: tested?.form === form ? tested.result : null,
    trustDialogState,
    updateForm,
  };
}
