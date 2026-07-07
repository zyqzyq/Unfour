import { redactSshLog } from "./helpers";
import {
  mockHostKeyFingerprintKey,
  mockState,
  mockStore,
  trimMockSshHistory,
} from "./state";
import { UNHANDLED, type MockResult } from "./types";
import type {
  SshCloseInput,
  SshConnectInput,
  SshConnection,
  SshConnectionInput,
  SshHostKeyInput,
  SshKnownHostsExportInput,
  SshKnownHostsImportInput,
  SshLogExport,
  SshLogExportInput,
  SshReconnectCancelInput,
  SshResizeInput,
  SshSessionEvent,
  SshSessionInput,
  SshSessionSummary,
  SshTestResult,
} from "../../types";

export function handleSshMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "ssh_connections_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockStore.sshConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "ssh_connection_save") {
    const input = args?.input as SshConnectionInput;
    const now = new Date().toISOString();
    const existingIndex = input.id
      ? mockStore.sshConnections.findIndex((item) => item.id === input.id)
      : -1;
    const connection: SshConnection = {
      id: input.id || crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name.trim(),
      host: input.host.trim(),
      port: input.port || 22,
      username: input.username.trim(),
      authKind: input.authKind,
      keyPath: input.keyPath?.trim() || null,
      credentialRef: input.credentialRef?.trim() || null,
      createdAt:
        existingIndex >= 0 ? mockStore.sshConnections[existingIndex].createdAt : now,
      updatedAt: now,
      deletedAt: null,
      revision:
        existingIndex >= 0 ? mockStore.sshConnections[existingIndex].revision + 1 : 1,
      syncStatus: existingIndex >= 0 ? "pending" : "local",
      remoteId: null,
    };
    if (existingIndex >= 0) {
      mockStore.sshConnections[existingIndex] = connection;
    } else {
      mockStore.sshConnections = [connection, ...mockStore.sshConnections];
    }
    return connection as T;
  }

  if (command === "ssh_connection_test") {
    const input = args?.input as SshConnectionInput;
    const host = input.host?.trim() ?? "";
    const username = input.username?.trim() ?? "";
    if (!host || !username) {
      return {
        ok: false,
        message: "Host and username are required",
      } satisfies SshTestResult as T;
    }
    return {
      ok: true,
      message: `Connected to ${username}@${host} successfully`,
    } satisfies SshTestResult as T;
  }

  if (command === "ssh_connection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const connectionId = String(args?.connectionId ?? "");
    mockStore.sshConnections = mockStore.sshConnections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === connectionId),
    );
    return mockStore.sshConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "ssh_session_connect") {
    const input = args?.input as SshConnectInput;
    const connection = mockStore.sshConnections.find(
      (item) => item.workspaceId === input.workspaceId && item.id === input.connectionId,
    );
    if (!connection) throw new Error("ssh connection not found");
    const now = new Date().toISOString();
    const session: SshSessionSummary = {
      sessionId: crypto.randomUUID(),
      workspaceId: input.workspaceId,
      connectionId: input.connectionId,
      status: "connected",
      reconnectAttempt: 0,
      authKind: connection.authKind,
      host: connection.host,
      username: connection.username,
      cols: input.cols ?? 120,
      rows: input.rows ?? 32,
      createdAt: now,
      updatedAt: now,
    };
    mockStore.sshSessions = [session, ...mockStore.sshSessions];
    mockStore.sshEvents.push({
      sessionId: session.sessionId,
      kind: "output",
      data: `Connected to ${session.username}@${session.host} with ${session.authKind} auth. PTY ${session.cols}x${session.rows} allocated.\r\n`,
      createdAt: now,
    });
    trimMockSshHistory(session.sessionId);
    // Simulate TOFU: record a mock fingerprint if not already stored.
    const hostKey = mockHostKeyFingerprintKey(
      connection.workspaceId,
      connection.host,
      connection.port,
    );
    if (!(hostKey in mockStore.hostKeyFingerprints)) {
      mockStore.hostKeyFingerprints[hostKey] = {
        workspaceId: connection.workspaceId,
        host: connection.host,
        port: connection.port,
        fingerprint: `SHA256:mock-${crypto.randomUUID().slice(0, 12)}`,
        createdAt: now,
      };
    }
    return session as T;
  }

  if (command === "ssh_sessions_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockStore.sshSessions.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "ssh_session_input") {
    const input = args?.input as SshSessionInput;
    const session = mockStore.sshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    if (session.status !== "connected") throw new Error("ssh session is not connected");
    const now = new Date().toISOString();
    mockStore.sshEvents.push({
      sessionId: input.sessionId,
      kind: "input",
      data: redactSshLog(input.data),
      createdAt: now,
    });
    const event: SshSessionEvent = {
      sessionId: input.sessionId,
      kind: "output",
      data: "Input accepted by SSH PTY stream.\r\n",
      createdAt: now,
    };
    mockStore.sshEvents.push(event);
    trimMockSshHistory(input.sessionId);
    session.updatedAt = now;
    return event as T;
  }

  if (command === "ssh_session_resize") {
    const input = args?.input as SshResizeInput;
    const session = mockStore.sshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.cols = input.cols;
    session.rows = input.rows;
    session.updatedAt = now;
    const event: SshSessionEvent = {
      sessionId: input.sessionId,
      kind: "resize",
      data: `PTY resized to ${input.cols}x${input.rows}.\r\n`,
      createdAt: now,
    };
    mockStore.sshEvents.push(event);
    return event as T;
  }

  if (command === "ssh_session_close") {
    const input = args?.input as SshCloseInput;
    const session = mockStore.sshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.status = "disconnected";
    session.reconnectAttempt = 0;
    session.updatedAt = now;
    mockStore.sshEvents.push({
      sessionId: input.sessionId,
      kind: "close",
      data: "SSH session closed.\r\n",
      createdAt: now,
    });
    return session as T;
  }

  if (command === "ssh_session_history") {
    const input = args?.input as SshCloseInput;
    const session = mockStore.sshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) return [] as T;
    return mockStore.sshEvents
      .filter((event) => event.sessionId === input.sessionId && event.kind !== "input")
      .map((event) => ({ ...event, data: redactSshLog(event.data) })) as T;
  }

  if (command === "ssh_session_reconnect_cancel") {
    const input = args?.input as SshReconnectCancelInput;
    const session = mockStore.sshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.status = "disconnected";
    session.reconnectAttempt = 0;
    session.updatedAt = now;
    mockStore.sshEvents.push({
      sessionId: input.sessionId,
      kind: "close",
      data: "SSH reconnect cancelled.\r\n",
      createdAt: now,
    });
    return session as T;
  }

  if (command === "ssh_session_log_export") {
    const input = args?.input as SshLogExportInput;
    const events = mockStore.sshEvents.filter((item) => item.sessionId === input.sessionId);
    const content = events
      .map((event) => `[${event.createdAt}] ${event.kind} ${redactSshLog(event.data)}`)
      .join("\n");
    return ({
      sessionId: input.sessionId,
      filename: `ssh-session-${input.sessionId}.log`,
      content,
      lineCount: events.length,
      redacted: content.includes("<redacted>"),
    } satisfies SshLogExport) as T;
  }

  if (command === "ssh_host_key_get") {
    const input = args?.input as SshHostKeyInput;
    const key = mockHostKeyFingerprintKey(input.workspaceId, input.host, input.port);
    const info = mockStore.hostKeyFingerprints[key];
    return (info ?? null) as T;
  }

  if (command === "ssh_host_key_reset") {
    const input = args?.input as SshHostKeyInput;
    const key = mockHostKeyFingerprintKey(input.workspaceId, input.host, input.port);
    const existed = key in mockStore.hostKeyFingerprints;
    delete mockStore.hostKeyFingerprints[key];
    return existed as T;
  }

  if (command === "ssh_host_key_list") {
    const workspaceId = args?.workspaceId as string;
    return Object.values(mockStore.hostKeyFingerprints).filter(
      (entry) => entry.workspaceId === workspaceId,
    ) as T;
  }

  if (command === "ssh_known_hosts_import") {
    const input = args?.input as SshKnownHostsImportInput;
    const lines = input.content.split("\n");
    let imported = 0;
    let skipped = 0;
    const errors: string[] = [];
    const now = new Date().toISOString();
    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#")) continue;
      const parts = line.split(/\s+/);
      if (
        parts.length < 3 ||
        (!parts[1].startsWith("ssh-") && !parts[1].startsWith("ecdsa-"))
      ) {
        skipped++;
        continue;
      }
      const hostField = parts[0];
      let host: string;
      let port = 22;
      if (hostField.startsWith("[")) {
        const bracketEnd = hostField.indexOf("]");
        if (bracketEnd < 0) {
          skipped++;
          continue;
        }
        host = hostField.slice(1, bracketEnd);
        const rest = hostField.slice(bracketEnd + 1);
        if (rest.startsWith(":")) port = parseInt(rest.slice(1), 10) || 22;
      } else {
        host = hostField;
      }
      const key = mockHostKeyFingerprintKey(input.workspaceId, host, port);
      if (key in mockStore.hostKeyFingerprints) {
        skipped++;
        continue;
      }
      mockStore.hostKeyFingerprints[key] = {
        workspaceId: input.workspaceId,
        host,
        port,
        fingerprint: `SHA256:mock-${crypto.randomUUID().slice(0, 12)}`,
        createdAt: now,
      };
      imported++;
    }
    return { imported, skipped, errors } as T;
  }

  if (command === "ssh_known_hosts_export") {
    const input = args?.input as SshKnownHostsExportInput;
    const entries = Object.values(mockStore.hostKeyFingerprints).filter(
      (entry) => entry.workspaceId === input.workspaceId,
    );
    const lines = entries.map((entry) => {
      const hostPort = entry.port === 22 ? entry.host : `[${entry.host}]:${entry.port}`;
      return `# ${hostPort} ${entry.fingerprint} (fingerprint only, no key data)`;
    });
    return {
      content: lines.length > 0 ? lines.join("\n") + "\n" : "",
      entryCount: 0,
    } as T;
  }

  return UNHANDLED;
}
