import { useMutation } from "@tanstack/react-query";
import { executeDatabaseQuery } from "@unfour/command-client";
import { useFeedback, useI18n } from "@unfour/ui";
import { sqlResultRowLimit } from "../model/result-row-limit";
import { isConfirmationRequired } from "../result-utils";

export type RunSqlParams = {
  confirmMutation: boolean;
  sql: string;
  catalog?: string | null;
  schema?: string | null;
  /** Client retention cap. Omitted values use the shared default. */
  limit?: number;
};

export function useSqlExecution({
  connectionId,
  onConfirmationRequired,
  onError,
  onExecuteStart,
  onSuccess,
  workspaceId,
}: {
  connectionId: string | null;
  onConfirmationRequired: (required: boolean) => void;
  onError?: (error: unknown, confirmMutation: boolean) => void;
  onExecuteStart: () => void;
  onSuccess: ReturnType<typeof executeDatabaseQuery> extends Promise<infer Result>
    ? (result: Result, confirmMutation: boolean) => void
    : never;
  workspaceId: string;
}) {
  const feedback = useFeedback();
  const { t } = useI18n();
  return useMutation({
    onMutate: onExecuteStart,
    mutationFn: ({ catalog, confirmMutation, limit, schema, sql }: RunSqlParams) =>
      executeDatabaseQuery({
        workspaceId,
        connectionId: connectionId ?? "",
        sql,
        limit: sqlResultRowLimit(limit),
        confirmMutation,
        catalog,
        schema,
      }),
    onError: (error, variables) => {
      onConfirmationRequired(isConfirmationRequired(error));
      onError?.(error, variables.confirmMutation);
    },
    onSuccess: (result, variables) => {
      onConfirmationRequired(false);
      onSuccess(result, variables.confirmMutation);
      if (result.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
        feedback.info(t("feedback.activityRecordFailed"));
      }
    },
  });
}
