import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  deleteApiRequest,
  duplicateApiRequest,
  updateApiRequest,
  type ApiSavedRequest,
} from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";
import { savedRequestToInput } from "../request-utils";

/** Sidebar CRUD only invalidates saved data; it does not replace open drafts. */
export function useApiCollectionRequestActions(workspaceId: string) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const invalidateSaved = () =>
    queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });

  const duplicateMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: invalidateSaved,
    onError: (error) => handleError(error, { key: "feedback.api.requestDuplicateFailed" }),
  });
  const deleteMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: invalidateSaved,
    onError: (error) => handleError(error, { key: "feedback.api.requestDeleteFailed" }),
  });
  const updateRequestMutation = useMutation({
    mutationFn: ({ name, request }: { name: string; request: ApiSavedRequest }) =>
      updateApiRequest(workspaceId, request.id, {
        ...savedRequestToInput(request, workspaceId),
        name,
      }),
    onSuccess: invalidateSaved,
    onError: (error) => handleError(error, { key: "feedback.api.requestRenameFailed" }),
  });

  return { duplicateMutation, deleteMutation, updateRequestMutation };
}
