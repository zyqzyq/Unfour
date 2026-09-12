import { defaultSql } from "./database-state";

export function normalizeQuerySql(sql: string) {
  return sql.trim();
}

/**
 * Query tabs confirm on close only when the editor has user-authored SQL that
 * differs from the tab's creation baseline. Empty SQL and unmodified default
 * or initially loaded SQL can close without a prompt.
 */
export function queryTabHasUnsavedDraft(tab: { sql: string; sqlBaseline?: string | null }) {
  const current = normalizeQuerySql(tab.sql);
  if (!current) {
    return false;
  }
  return current !== normalizeQuerySql(tab.sqlBaseline ?? defaultSql);
}
