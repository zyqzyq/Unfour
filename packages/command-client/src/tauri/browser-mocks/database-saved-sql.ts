import type { SavedSql, SavedSqlInput } from "../../types";
import { mockStore } from "./state";

export function listMockSavedSql(workspaceId: string) {
  return mockStore.savedSql.filter((item) => item.workspaceId === workspaceId);
}

export function saveMockSavedSql(input: SavedSqlInput) {
  const workspaceId = input.workspaceId;
  const id = input.id?.trim();
  const { name, sql } = savedSqlContent(input);
  const now = new Date().toISOString();
  const existingIndex = id
    ? mockStore.savedSql.findIndex((item) => item.id === id && item.workspaceId === workspaceId)
    : -1;
  if (id && existingIndex === -1) throw new Error("saved SQL not found");
  const saved: SavedSql = {
    id: id || crypto.randomUUID(),
    workspaceId,
    connectionId: input.connectionId ?? null,
    catalog: input.catalog?.trim() || null,
    schema: input.schema?.trim() || null,
    name,
    sql,
    createdAt: existingIndex >= 0 ? mockStore.savedSql[existingIndex].createdAt : now,
    updatedAt: now,
  };
  if (existingIndex >= 0) {
    mockStore.savedSql = [
      saved,
      ...mockStore.savedSql.filter((_item, index) => index !== existingIndex),
    ];
  } else {
    mockStore.savedSql = [saved, ...mockStore.savedSql];
  }
  return saved;
}

function savedSqlContent(input: SavedSqlInput) {
  const name = input.name.trim();
  const sql = input.sql.trim();
  if (!name) throw new Error("saved SQL name cannot be empty");
  if (name.length > 120) throw new Error("saved SQL name must be 120 characters or fewer");
  if (!sql) throw new Error("saved SQL cannot be empty");
  return { name, sql };
}

export function deleteMockSavedSql(workspaceId: string, id: string) {
  const initialLength = mockStore.savedSql.length;
  mockStore.savedSql = mockStore.savedSql.filter(
    (item) => !(item.workspaceId === workspaceId && item.id === id),
  );
  if (mockStore.savedSql.length === initialLength) throw new Error("saved SQL not found");
  return mockStore.savedSql.filter((item) => item.workspaceId === workspaceId);
}
