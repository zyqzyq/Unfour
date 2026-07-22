import { executeDatabaseQuery, type DatabaseQueryResult } from "@unfour/command-client";
import { isConfirmationRequired } from "../result-utils";

export type SqlBatchState = {
  catalog: string | null;
  collected: DatabaseQueryResult[];
  connectionId: string;
  nextIndex: number;
  schema: string | null;
  statements: string[];
  tabId: string;
};

type ExecuteSqlBatchDeps = {
  cancelled: () => boolean;
  onConfirmationRequired: (batch: SqlBatchState, collected: DatabaseQueryResult[], error: unknown) => void;
  onError: (batch: SqlBatchState, collected: DatabaseQueryResult[], sql: string, error: unknown) => void;
  onStatementSuccess: (
    batch: SqlBatchState,
    collected: DatabaseQueryResult[],
    sql: string,
    result: DatabaseQueryResult,
  ) => void;
  onSuccess: (batch: SqlBatchState, collected: DatabaseQueryResult[]) => void;
  workspaceId: string;
};

/**
 * Sequentially execute single-statement backend calls for a Run Current / Run All
 * script. Pauses on CONFIRMATION_REQUIRED so the UI can resume with confirmMutation.
 */
export async function executeSqlBatch(
  batch: SqlBatchState,
  confirmMutation: boolean,
  deps: ExecuteSqlBatchDeps,
): Promise<"completed" | "confirmation" | "error" | "cancelled"> {
  const collected = [...batch.collected];
  // After the user confirms once in a script, keep confirming later writes
  // in the same Run All so a migration is not interrupted per statement.
  const confirmRemaining = confirmMutation;

  for (let index = batch.nextIndex; index < batch.statements.length; index += 1) {
    if (deps.cancelled()) {
      return "cancelled";
    }

    const sql = batch.statements[index]!;
    const current: SqlBatchState = { ...batch, collected, nextIndex: index };

    try {
      const result = await executeDatabaseQuery({
        workspaceId: deps.workspaceId,
        connectionId: batch.connectionId,
        sql,
        limit: 100,
        confirmMutation: confirmRemaining,
        catalog: batch.catalog,
        schema: batch.schema,
      });

      if (deps.cancelled()) {
        return "cancelled";
      }

      collected.push(result);
      deps.onStatementSuccess(current, collected, sql, result);
    } catch (error) {
      if (deps.cancelled()) {
        return "cancelled";
      }

      if (isConfirmationRequired(error)) {
        deps.onConfirmationRequired({ ...batch, collected, nextIndex: index }, collected, error);
        return "confirmation";
      }

      deps.onError(current, collected, sql, error);
      return "error";
    }
  }

  if (deps.cancelled()) {
    return "cancelled";
  }

  deps.onSuccess(batch, collected);
  return "completed";
}
