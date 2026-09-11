import { executeDatabaseScript, type DatabaseScriptInput } from "@unfour/command-client";
import type { DatabaseQueryWorkspaceTab, RunSqlOptions } from "./types";

export type SqlBatchState = {
  tabId: string;
  source: string;
  input: DatabaseScriptInput;
};

/** Preserve source SQL; all dialect parsing and preflight live in Rust. */
export function createSqlBatch(tab: DatabaseQueryWorkspaceTab, options: RunSqlOptions, workspaceId: string): SqlBatchState {
  return {
    tabId: tab.id,
    source: tab.sql,
    input: {
      workspaceId,
      connectionId: tab.connectionId ?? "",
      catalog: tab.catalog,
      schema: tab.schema,
      sql: options.sql ?? tab.sql,
      cursorOffset: options.sql === undefined && options.mode !== "all" ? options.cursorOffset ?? 0 : undefined,
      runId: crypto.randomUUID(),
      explain: options.explain,
      limit: 100,
      confirmMutation: false,
    },
  };
}

export function canConfirmBatch(batch: SqlBatchState, tab: DatabaseQueryWorkspaceTab, workspaceId: string) {
  return batch.tabId === tab.id && batch.source === tab.sql &&
    batch.input.workspaceId === workspaceId && batch.input.connectionId === tab.connectionId &&
    batch.input.catalog === tab.catalog && batch.input.schema === tab.schema;
}

export function executeSqlBatch(batch: SqlBatchState, confirmMutation: boolean) {
  return executeDatabaseScript({ ...batch.input, confirmMutation });
}

export function sqlAwaitingConfirmation(batch: SqlBatchState, error: unknown): string {
  const details = error && typeof error === "object" && "details" in error ? error.details : null;
  const ranges = details && typeof details === "object" && "statementRanges" in details ? details.statementRanges : null;
  if (!Array.isArray(ranges)) return batch.input.sql;
  const sql = ranges.flatMap((range: unknown) => {
    if (!range || typeof range !== "object" || !("start" in range) || !("end" in range)) return [];
    if (typeof range.start !== "number" || typeof range.end !== "number") return [];
    return [batch.input.sql.slice(range.start, range.end)];
  }).join("\n");
  return batch.input.explain ? `EXPLAIN ${sql}` : sql;
}
