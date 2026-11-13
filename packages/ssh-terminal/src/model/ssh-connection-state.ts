import { connectionTemporaryHandle, type SshConnection, type SshConnectionInput } from "@unfour/command-client";

export function sshConnectionContent(connection: SshConnection | null) {
  return connection ? JSON.stringify({ ...sshConnectionToInput(connection, connection.workspaceId), expectedSnapshot: undefined }) : null;
}

export function defaultSshConnectionInput(workspaceId: string): SshConnectionInput {
  return {
    workspaceId,
    name: "",
    host: "",
    port: 22,
    username: "",
    authKind: "password",
    credentialRef: null,
    secret: null,
  };
}

export function sshConnectionToInput(
  connection: SshConnection,
  workspaceId: string,
): SshConnectionInput {
  return {
    id: connection.id,
    temporaryHandle: connectionTemporaryHandle("ssh", workspaceId, connection.id) ?? null,
    expectedSnapshot: connection,
    workspaceId,
    name: connection.name,
    host: connection.host,
    port: connection.port,
    username: connection.username,
    authKind: connection.authKind,
    keyPath: connection.keyPath,
    credentialRef: connection.credentialRef,
    // Never surface the stored secret; blank means "keep the saved password".
    secret: null,
  };
}

export function sshEndpointLabel(connection: SshConnection | null | undefined) {
  if (!connection) {
    return "No SSH connection";
  }

  return `${connection.username}@${connection.host}`;
}
