import type { DatabaseTable } from "@unfour/command-client";

export function databaseTableTreeId(connectionId: string, table: DatabaseTable) {
  return `${connectionId}:table:${table.schema ?? "default"}:${table.name}`;
}
