import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { updateWorkspaceLayout } from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";

export function useLayoutPersistence(activeWorkspaceId: string | null) {
  const {
    activeTabId,
    bottomPanelHeight,
    layoutWorkspaceId,
    rightInspectorWidth,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    sidebarCollapsed,
    sidebarWidths,
    snapshotLayout,
    tabs,
  } = useWorkspaceStore();
  const handleError = useFeedbackErrorHandler();

  const layoutMutation = useMutation({
    mutationFn: (workspaceId: string) =>
      updateWorkspaceLayout(workspaceId, snapshotLayout(workspaceId)),
    onError: (error) => handleError(error, { key: "feedback.layout.saveFailed" }),
  });

  // React Query's mutate function is stable; the result object is not.
  const persistLayout = layoutMutation.mutate;

  useEffect(() => {
    if (!activeWorkspaceId || layoutWorkspaceId !== activeWorkspaceId) {
      return;
    }

    const timeout = window.setTimeout(() => {
      persistLayout(activeWorkspaceId);
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [
    activeTabId,
    activeWorkspaceId,
    layoutWorkspaceId,
    persistLayout,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    sidebarCollapsed,
    sidebarWidths,
    bottomPanelHeight,
    rightInspectorWidth,
    tabs,
  ]);

  return { layoutMutation, snapshotLayout };
}
