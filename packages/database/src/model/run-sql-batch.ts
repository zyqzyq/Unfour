import { executeDatabaseScript } from "@unfour/command-client";
import type { DatabaseQueryWorkspaceTab, RunSqlOptions, SqlBatchState } from "./types";

export type { SqlBatchState };

/** Preserve source SQL; all dialect parsing and preflight live in Rust. */
export function createSqlBatch(tab: DatabaseQueryWorkspaceTab, options: RunSqlOptions, workspaceId: string): SqlBatchState {
  if (options.resume || options.cancelConfirmation) {
    throw new Error("Confirmation control requests cannot create a new SQL batch");
  }
  const runAll = options.mode === "all";
  return {
    tabId: tab.id,
    source: tab.sql,
    input: {
      workspaceId,
      connectionId: tab.connectionId ?? "",
      catalog: tab.catalog,
      schema: tab.schema,
      sql: runAll ? tab.sql : (options.sql ?? tab.sql),
      cursorOffset: runAll || options.sql !== undefined ? undefined : options.cursorOffset ?? 0,
      runId: crypto.randomUUID(),
      explain: options.explain,
      limit: 100,
      confirmMutation: false,
    },
  };
}

export function canConfirmBatch(batch: SqlBatchState, tab: DatabaseQueryWorkspaceTab, workspaceId: string) {
  return batch.tabId === tab.id && batch.source === tab.sql &&
    batch.input.workspaceId === workspaceId && batch.input.connectionId === (tab.connectionId ?? "") &&
    sameNullableText(batch.input.catalog, tab.catalog) && sameNullableText(batch.input.schema, tab.schema);
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

function sameNullableText(left: string | null | undefined, right: string | null | undefined) {
  return (left ?? null) === (right ?? null);
}
