import { Channel, invoke } from "@tauri-apps/api/core";
import { call, isTauriRuntime } from "./invoke";
import type {
  SshCloseInput,
  SshConnectInput,
  SshConnection,
  SshConnectionInput,
  SshHostFingerprintInfo,
  SshHostKeyInput,
  SshKnownHostsExportInput,
  SshKnownHostsExportResult,
  SshKnownHostsImportInput,
  SshKnownHostsImportResult,
  SshLogExport,
  SshLogExportInput,
  SshReconnectCancelInput,
  SshResizeInput,
  SshSessionEvent,
  SshSessionInput,
  SshSessionSummary,
  SshTestResult,
} from "../types";

export function listSshConnections(workspaceId: string) {
  return call<SshConnection[]>("ssh_connections_list", { workspaceId });
}

export function saveSshConnection(input: SshConnectionInput) {
  return call<SshConnection>("ssh_connection_save", { input });
}

export function testSshConnection(input: SshConnectionInput) {
  return call<SshTestResult>("ssh_connection_test", { input });
}

export function deleteSshConnection(workspaceId: string, connectionId: string) {
  return call<SshConnection[]>("ssh_connection_delete", {
    workspaceId,
    connectionId,
  });
}

export function connectSshSession(input: SshConnectInput) {
  return call<SshSessionSummary>("ssh_session_connect", { input });
}

export function listSshSessions(workspaceId: string) {
  return call<SshSessionSummary[]>("ssh_sessions_list", { workspaceId });
}

export function getSshSessionHistory(input: SshCloseInput) {
  return call<SshSessionEvent[]>("ssh_session_history", { input });
}

export function sendSshInput(input: SshSessionInput) {
  return call<SshSessionEvent>("ssh_session_input", { input });
}

export function resizeSshSession(input: SshResizeInput) {
  return call<SshSessionEvent>("ssh_session_resize", { input });
}

export function closeSshSession(input: SshCloseInput) {
  return call<SshSessionSummary>("ssh_session_close", { input });
}

export function cancelSshReconnect(input: SshReconnectCancelInput) {
  return call<SshSessionSummary>("ssh_session_reconnect_cancel", { input });
}

export function exportSshLog(input: SshLogExportInput) {
  return call<SshLogExport>("ssh_session_log_export", { input });
}

export function getSshHostFingerprint(input: SshHostKeyInput) {
  return call<SshHostFingerprintInfo | null>("ssh_host_key_get", { input });
}

export function resetSshHostFingerprint(input: SshHostKeyInput) {
  return call<boolean>("ssh_host_key_reset", { input });
}

export function listSshHostFingerprints(workspaceId: string) {
  return call<SshHostFingerprintInfo[]>("ssh_host_key_list", { workspaceId });
}

export function importSshKnownHosts(input: SshKnownHostsImportInput) {
  return call<SshKnownHostsImportResult>("ssh_known_hosts_import", { input });
}

export function exportSshKnownHosts(input: SshKnownHostsExportInput) {
  return call<SshKnownHostsExportResult>("ssh_known_hosts_export", { input });
}

export type SshTerminalDataPayload = {
  sessionId: string;
  data: string;
  status?: SshSessionSummary["status"] | null;
  reconnectAttempt?: number;
};

/**
 * Subscribe to live SSH terminal output over a Tauri IPC `Channel` (the same
 * reliable transport used by commands) rather than the event system. High-rate
 * Tauri events stall WebView2 event delivery on Windows under a full-screen
 * redraw burst (vim/less/top), which silently freezes the terminal; channels
 * do not have that failure mode. Returns a disposer that detaches the handler.
 * A no-op outside the Tauri runtime (browser mock mode polls history instead).
 */
export function registerSshTerminalChannel(
  onMessage: (payload: SshTerminalDataPayload) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return Promise.resolve(() => {});
  }
  const channel = new Channel<SshTerminalDataPayload>();
  channel.onmessage = onMessage;
  return invoke<void>("ssh_register_terminal_channel", { channel })
    .then(() => () => {
      channel.onmessage = () => {};
    })
    .catch(() => () => {
      channel.onmessage = () => {};
    });
}
