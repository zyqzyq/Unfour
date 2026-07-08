import { useEffect, useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import { updateWorkspaceLayout } from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";

export function useLayoutPersistence(activeWorkspaceId: string | null) {
  const {
    activeTabId,
    layoutWorkspaceId,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    sidebarCollapsed,
    snapshotLayout,
    tabs,
  } = useWorkspaceStore();
  const handleError = useFeedbackErrorHandler();

  const layoutMutation = useMutation({
    mutationFn: (workspaceId: string) =>
      updateWorkspaceLayout(workspaceId, snapshotLayout(workspaceId)),
    onError: (error) => handleError(error, { key: "feedback.layout.saveFailed" }),
  });

  // Keep a stable ref to the mutate function so the debounced effect
  // does not re-trigger on every render (layoutMutation object identity
  // changes each render even though .mutate is stable).
  const mutateRef = useRef(layoutMutation.mutate);
  // eslint-disable-next-line react-hooks/refs -- render-time ref sync is the recommended pattern for stabilizing callbacks
  mutateRef.current = layoutMutation.mutate;

  useEffect(() => {
    if (!activeWorkspaceId || layoutWorkspaceId !== activeWorkspaceId) {
      return;
    }

    const timeout = window.setTimeout(() => {
      mutateRef.current(activeWorkspaceId);
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [
    activeTabId,
    activeWorkspaceId,
    layoutWorkspaceId,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    sidebarCollapsed,
    tabs,
  ]);

  return { layoutMutation, snapshotLayout };
}
