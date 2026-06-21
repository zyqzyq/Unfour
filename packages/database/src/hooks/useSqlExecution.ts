import { useMutation } from "@tanstack/react-query";
import { executeDatabaseQuery } from "@unfour/command-client";
import { isConfirmationRequired } from "../result-utils";

export type RunSqlParams = {
  confirmMutation: boolean;
  sql: string;
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
  return useMutation({
    onMutate: onExecuteStart,
    mutationFn: ({ confirmMutation, sql }: RunSqlParams) =>
      executeDatabaseQuery({
        workspaceId,
        connectionId: connectionId ?? "",
        sql,
        limit: 100,
        confirmMutation,
      }),
    onError: (error, variables) => {
      onConfirmationRequired(isConfirmationRequired(error));
      onError?.(error, variables.confirmMutation);
    },
    onSuccess: (result, variables) => {
      onConfirmationRequired(false);
      onSuccess(result, variables.confirmMutation);
    },
  });
}
