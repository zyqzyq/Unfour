import { useMutation } from "@tanstack/react-query";
import { executeDatabaseQuery } from "@unfour/command-client";
import { isConfirmationRequired } from "../result-utils";

export function useSqlExecution({
  connectionId,
  onConfirmationRequired,
  onError,
  onExecuteStart,
  onSuccess,
  sql,
  workspaceId,
}: {
  connectionId: string | null;
  onConfirmationRequired: (required: boolean) => void;
  onError?: (error: unknown, confirmMutation: boolean) => void;
  onExecuteStart: () => void;
  onSuccess: ReturnType<typeof executeDatabaseQuery> extends Promise<infer Result>
    ? (result: Result, confirmMutation: boolean) => void
    : never;
  sql: string;
  workspaceId: string;
}) {
  return useMutation({
    onMutate: onExecuteStart,
    mutationFn: (confirmMutation: boolean) =>
      executeDatabaseQuery({
        workspaceId,
        connectionId: connectionId ?? "",
        sql,
        limit: 100,
        confirmMutation,
      }),
    onError: (error, confirmMutation) => {
      onConfirmationRequired(isConfirmationRequired(error));
      onError?.(error, confirmMutation);
    },
    onSuccess: (result, confirmMutation) => {
      onConfirmationRequired(false);
      onSuccess(result, confirmMutation);
    },
  });
}
