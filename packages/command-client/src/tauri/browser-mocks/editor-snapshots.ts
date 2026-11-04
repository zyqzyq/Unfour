import type { DatabaseConnection, SshConnection, SshTaskDetail } from "../../types";

export function connectionEditSnapshot(connection: DatabaseConnection | SshConnection | undefined) {
  if (!connection) return null;
  const { createdAt: _created, updatedAt: _updated, deletedAt: _deleted, revision: _revision, syncStatus: _sync, remoteId: _remote, ...content } = connection;
  return JSON.stringify(content);
}

export function taskEditSnapshot(detail: SshTaskDetail | undefined) {
  return detail ? JSON.stringify({
    id: detail.task.id, workspaceId: detail.task.workspaceId, name: detail.task.name,
    description: detail.task.description, defaultConnectionId: detail.localBinding?.defaultConnectionId ?? null,
    steps: detail.steps.map(({ id, name, stepType, position, enabled, configVersion, configJson }) => ({ id, name, stepType, position, enabled, configVersion, configJson })),
  }) : null;
}
