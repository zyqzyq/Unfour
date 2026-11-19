import { useFeedback, useI18n, createTranslator, type TFunction } from "@unfour/ui";
import { multipartRuntimeInput, multipartValidation } from "../model/multipart";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteApiRequest,
  cancelApiRequest,
  duplicateApiRequest,
  getApiHistoryDetail,
  listWorkspaceEnvironments,
  listApiHistory,
  listSavedApiRequests,
  saveApiRequest,
  sendApiRequest,
  updateApiRequest,
  type ApiRequestInput,
  type ApiRouteEdit,
  type ApiSavedRequest,
  type KeyValue,
  type TemporarySshRoute,
} from "@unfour/command-client";
import { errorCode, formatError, shouldAppendOutcomeWarning } from "../model/api-request-state";
import { isTerminalApiError, terminalApiSendError } from "../model/terminal-api";
import { effectiveApiRoute, environmentRouteMutationKey, useApiRoutes } from "./useApiRoutes";
import { useTerminalApiConfirmation } from "./useTerminalApiConfirmation";
import { historyReplayError, localizeReplayError } from "../model/history-replay";
import {
  DEFAULT_API_TAB_STATE,
  useApiRequestTabStore,
} from "../model/api-request-tab-state";
import type { ApiRequestTab } from "../model/request-tabs";
import {
  addHeaderIfMissing,
  bodyFieldsToInput,
  sendableKeyValues,
  stripUrlQuery,
} from "../request-utils";
import type {
  ApiSplitDirection,
  ApiAuthConfig,
  RequestDraft,
  RequestParamsTab,
  ResponseTab,
} from "../model/types";

export function useApiRequestTabs(workspaceId: string) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const queryClient = useQueryClient();
  const [collectionStatus, setCollectionStatus] = useState("");
  const state = useApiRequestTabStore(
    (s) => s.byWorkspace[workspaceId] ?? DEFAULT_API_TAB_STATE,
  );

  const savedQuery = useQuery({
    networkMode: "always",
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["api-requests"] },
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const historyQuery = useQuery({
    networkMode: "always",
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });
  const environmentsQuery = useQuery({
    networkMode: "always",
    enabled: Boolean(workspaceId),
    queryKey: ["workspace-environments", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["environments"] },
    queryFn: () => listWorkspaceEnvironments(workspaceId),
  });

  const savingTabs = state.tabs.filter((tab) => tab.saving).map((tab) => tab.id).join("|");
  useEffect(() => {
    if (savedQuery.data) useApiRequestTabStore.getState().reconcileSaved(workspaceId, savedQuery.data);
  }, [savedQuery.data, workspaceId, savingTabs]);

  function refreshRouteContext(workspace: string) {
    for (const family of ["api-routes", "workspace-environments", "ssh-connections"]) {
      void queryClient.invalidateQueries({ queryKey: [family, workspace] });
    }
  }

  function sendErrorMessage(workspace: string, code: string | null, raw: string, failure: unknown, hasResponse = false) {
    let message = localizeReplayError(raw, t);
    if (code === "API_RESPONSE_TOO_LARGE") message = t("api.response.tooLarge");
    if (code === "API_CANCELLED") message = t("api.response.cancelledDescription");
    if (code?.startsWith("SSH_TUNNEL_") || isTerminalApiError(code)) {
      message = t(`network.errors.${code}`, t("api.route.failed"));
      refreshRouteContext(workspace);
    }
    if (!hasResponse && shouldAppendOutcomeWarning(code, failure)) {
      message = `${message} ${t("feedback.apiOutcomeUncertain")}`.trim();
    }
    return message;
  }

  const sendMutation = useMutation({
    // This dispatches local IPC. Only the backend can assess the remote route.
    networkMode: "always",
    retry: false,
    mutationFn: ({ executionId, input, routing, temporaryHandle, temporaryExecution }: { executionId: string; input: ApiRequestInput; tabId: string; routing: NonNullable<Parameters<typeof sendApiRequest>[2]>; temporaryHandle?: string; temporaryExecution?: Parameters<typeof sendApiRequest>[4] }) =>
      temporaryExecution ? sendApiRequest(executionId, input, routing, temporaryHandle, temporaryExecution)
        : temporaryHandle ? sendApiRequest(executionId, input, routing, temporaryHandle) : sendApiRequest(executionId, input, routing),
    onSuccess: (execution, variables) => {
      const workspaceId = variables.input.workspaceId;
      if (execution.httpError || execution.httpErrorCode) {
        execution = { ...execution, httpError: sendErrorMessage(workspaceId, execution.httpErrorCode, execution.httpError ?? "", execution, Boolean(execution.response)) };
      }
      if (execution.postResponse.error?.code === "API_POST_RESPONSE_FINALIZATION_FAILED") {
        execution = { ...execution, postResponse: { ...execution.postResponse, error: { ...execution.postResponse.error, message: t("feedback.apiPostResponseFailed") } } };
      }
      const store = useApiRequestTabStore.getState();
      const current = store.byWorkspace[workspaceId]?.tabs.find((tab) => tab.executionId === variables.executionId);
      if (current) store.completeTabSend(workspaceId, current.id, execution);
      const warnings = execution.warnings ?? execution.response?.warnings;
      if (execution.response && !execution.httpError && warnings?.includes("ACTIVITY_RECORD_FAILED")) {
        feedback.info(t("feedback.activityRecordFailed"));
      }
      if (execution.response && warnings?.includes("API_HISTORY_RECORD_FAILED")) {
        feedback.info(t("feedback.apiHistoryRecordFailed"));
      }
      if (execution.response && warnings?.includes("API_POST_RESPONSE_CANCELLED")) {
        feedback.info(t("feedback.apiPostResponseCancelled"));
      }
      if (execution.postResponse.error?.code === "API_POST_RESPONSE_FINALIZATION_FAILED") {
        feedback.info(t("feedback.apiPostResponseFailed"));
      }
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
      if (variables.temporaryHandle) void queryClient.invalidateQueries({ queryKey: ["api-live-sessions", workspaceId] });
    },
    onError: (error, variables) => {
      const workspaceId = variables.input.workspaceId;
      const store = useApiRequestTabStore.getState();
      const current = store.byWorkspace[workspaceId]?.tabs.find((tab) => tab.executionId === variables.executionId);
      const code = errorCode(error);
      const message = sendErrorMessage(workspaceId, code, formatError(error), error);
      if (current) store.failTabSend(workspaceId, current.id, message, code);
      if (variables.temporaryHandle) void queryClient.invalidateQueries({ queryKey: ["api-live-sessions", workspaceId] });
    },
  });

  const saveMutation = useMutation({
    mutationFn: ({
      input,
      requestId,
      expectedRevision,
      route,
    }: {
      input: ApiRequestInput;
      requestId?: string | null;
      tabId: string;
      expectedRevision?: number;
      route?: ApiRouteEdit;
    }) =>
      requestId
        ? updateApiRequest(input.workspaceId, requestId, input, expectedRevision, route)
        : saveApiRequest(input, route?.route),
    onSuccess: async (saved, variables) => {
      const workspaceId = variables.input.workspaceId;
      const queryKey = ["api-saved", workspaceId];
      // A list read started before the save must not restore the old definition.
      await queryClient.cancelQueries({ queryKey });
      if (variables.route) {
        // Refetch before releasing the submitted draft; cached routing from before Save
        // must not be presented, or sent, as the newly saved selection.
        await queryClient.resetQueries({ queryKey: ["api-routes", workspaceId, saved.id] });
      }
      queryClient.setQueryData<ApiSavedRequest[]>(queryKey, (current = []) => {
        const existing = current.find((item) => item.id === saved.id);
        if (existing && existing.revision > saved.revision) return current;
        return existing ? current.map((item) => item.id === saved.id ? saved : item) : [...current, saved];
      });
      useApiRequestTabStore.getState().completeTabSave(workspaceId, variables.tabId, saved);
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
      // Also invalidate collections in case a default was auto-created
      queryClient.invalidateQueries({ queryKey: ["api-collections", workspaceId] });
    },
    onError: (error, variables) => {
      const code = errorCode(error);
      const routeError = code?.startsWith("SSH_TUNNEL_");
      useApiRequestTabStore.getState().failTabSave(variables.input.workspaceId, variables.tabId,
        routeError ? t(`network.errors.${code}`, t("api.route.failed")) : code === "ROW_CONFLICT" ? t("api.external.updated") : formatError(error));
      if (routeError) refreshRouteContext(variables.input.workspaceId);
      queryClient.invalidateQueries({ queryKey: ["api-saved", variables.input.workspaceId] });
    },
  });

  const duplicateMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });
  const deleteMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: (_, requestId) => {
      useApiRequestTabStore.getState().closeTab(workspaceId, `saved:${requestId}`);
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
    },
  });
  const sendRequest = sendMutation.mutate;
  const saveRequest = saveMutation.mutateAsync;

  const activeTab = state.tabs.find((tab) => tab.id === state.activeTabId) ?? null;
  const environments = useMemo(
    () => environmentsQuery.data ?? [],
    [environmentsQuery.data],
  );
  const activeEnvironment = useMemo(
    () => environments.find((environment) => environment.isActive) ?? null,
    [environments],
  );
  const environmentId = activeEnvironment?.id ?? null;
  const routing = useApiRoutes(workspaceId, activeTab, environmentId, environmentsQuery.status);
  useTerminalApiConfirmation(workspaceId, activeTab, routing, environmentId);
  const newRequest = useCallback((draft?: RequestDraft) => {
    useApiRequestTabStore.getState().newRequest(workspaceId, draft);
  }, [workspaceId]);

  const openSaved = useCallback(
    (requestId: string) => {
      const saved = savedQuery.data?.find((item) => item.id === requestId);
      if (saved) {
        const store = useApiRequestTabStore.getState();
        store.openSaved(workspaceId, saved);
        return useApiRequestTabStore.getState().byWorkspace[workspaceId].tabs.find((tab) => tab.savedRequestId === requestId);
      }
    },
    [savedQuery.data, workspaceId],
  );

  const openHistory = useCallback(
    async (historyId: string) => {
      // Claim the workspace before the asynchronous history read completes.
      useApiRequestTabStore.getState().initializeWorkspace(workspaceId);
      const detail = await getApiHistoryDetail(workspaceId, historyId);
      useApiRequestTabStore.getState().openHistory(workspaceId, detail);
    },
    [workspaceId],
  );

  function updateDraft(tabId: string, patch: Partial<RequestDraft>) {
    useApiRequestTabStore.getState().updateTabDraft(workspaceId, tabId, patch);
  }

  const sendTab = useCallback(
    (tab: ApiRequestTab) => {
      const current = useApiRequestTabStore.getState().byWorkspace[workspaceId];
      const liveTab = current ? current.tabs.find((item) => item.id === tab.id) : tab;
      if (!liveTab || liveTab.sending || liveTab.cancelling) return;
      tab = liveTab;
      // Read the mutation cache at the execution boundary (also covers Retry
      // and keyboard Send), not a potentially stale rendered button state.
      if (queryClient.isMutating({ mutationKey: ["workspace-environment-activation", workspaceId] })) {
        useApiRequestTabStore.getState().failTabSend(workspaceId, tab.id, t("variables.environmentSwitchingSend"));
        return;
      }
      const validationError = validateBeforeSend(tab, t);
      if (validationError) {
        useApiRequestTabStore.getState().failTabSend(workspaceId, tab.id, validationError);
        return;
      }
      const routes = routing.routes;
      const liveSessions = queryClient.getQueryData<TemporarySshRoute[]>(["api-live-sessions", workspaceId]) ?? routing.temporary.data;
      const terminalError = terminalApiSendError(tab.draft, liveSessions?.find((entry) => entry.handle === tab.draft.temporaryHandle));
      if (terminalError) {
        useApiRequestTabStore.getState().updateTabDraft(workspaceId, tab.id, { terminalConfirmed: false });
        // A blocked retry must retain the original failure or received response.
        // The terminal notice already explains the recovery requirement.
        if (terminalError === "SSH_TERMINAL_RECOVERY_REQUIRED" && (
          isTerminalApiError(tab.sendErrorCode) || tab.response?.warnings?.includes("SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED")
        )) return;
        useApiRequestTabStore.getState().failTabSend(workspaceId, tab.id, t(`network.errors.${terminalError}`), terminalError);
        return;
      }
      if (!routing.ready || !routes || tab.id !== activeTab?.id || queryClient.isMutating({ mutationKey: environmentRouteMutationKey(workspaceId) })) {
        const message = tab.draft.temporaryHandle && !routing.temporary.data?.some((entry) => entry.handle === tab.draft.temporaryHandle)
          ? t("network.errors.SSH_TUNNEL_TEMPORARY_UNAVAILABLE") : t("api.route.unavailable");
        useApiRequestTabStore.getState().failTabSend(workspaceId, tab.id, message);
        return;
      }
      const input = tabToInput(tab, workspaceId, { purpose: "send" });
      const executionId = crypto.randomUUID();
      useApiRequestTabStore.getState().startTabSend(workspaceId, tab.id, input, executionId);
      sendRequest({ executionId, input, tabId: tab.id, temporaryHandle: tab.draft.temporaryHandle,
        ...(tab.draft.temporaryHandle && tab.draft.temporaryExecutionMode === "terminalCurl" && { temporaryExecution: { temporaryExecutionMode: "terminalCurl" as const, terminalConfirmed: true } }), routing: {
        requestId: tab.savedRequestId, route: tab.draft.route?.route,
        expectedEnvironmentId: routes.environmentId, expectedEffective: effectiveApiRoute(routes, tab.draft.route?.route),
        expectedSshRevision: routing.selectedHost?.revision,
      } });
    },
    [sendRequest, workspaceId, t, queryClient, activeTab?.id, routing.ready, routing.routes, routing.selectedHost?.revision, routing.temporary.data],
  );

  const cancelTab = useCallback(async (tab: ApiRequestTab) => {
    const live = useApiRequestTabStore.getState().byWorkspace[workspaceId]?.tabs.find((entry) => entry.id === tab.id);
    if (!tab.executionId || live?.executionId !== tab.executionId || !live.sending || live.cancelling) return;
    const executionId = tab.executionId;
    useApiRequestTabStore.getState().startTabCancel(workspaceId, tab.id);
    try {
      if (await cancelApiRequest(executionId)) return;
    } catch {
      // A failed IPC does not establish that the remote request has stopped.
    }
    const store = useApiRequestTabStore.getState();
    // Saving an unsaved request can replace its tab ID while cancellation is pending.
    const current = store.byWorkspace[workspaceId]?.tabs.find((entry) => entry.executionId === executionId);
    if (current) store.failTabCancel(workspaceId, current.id, executionId, t("api.response.cancelFailed"));
  }, [workspaceId, t]);

  const saveTab = useCallback(
    async (
      tab: ApiRequestTab,
      identity?: {
        collectionId: string | null;
        name: string;
        parentFolderId: string | null;
      },
    ) => {
      const current = useApiRequestTabStore.getState().byWorkspace[workspaceId];
      const liveTab = current ? current.tabs.find((item) => item.id === tab.id) : tab;
      if (!liveTab || liveTab.saving) return null;
      tab = liveTab;
      if (tab.externalChange) {
        useApiRequestTabStore.getState().failTabSave(workspaceId, tab.id, t(`api.external.${tab.externalChange}`));
        return null;
      }
      const replayError = historyReplayError(tab, t);
      if (replayError) {
        useApiRequestTabStore.getState().failTabSave(workspaceId, tab.id, replayError);
        return null;
      }
      const draft = identity ? { ...tab.draft, ...identity } : tab.draft;
      if (identity) {
        useApiRequestTabStore.getState().updateTabDraft(workspaceId, tab.id, identity);
      }
      useApiRequestTabStore.getState().startTabSave(workspaceId, tab.id);
      try {
        const saved = await saveRequest({
          input: tabToInput({ ...tab, draft }, workspaceId, { purpose: "save" }),
          requestId: tab.savedRequestId,
          tabId: tab.id,
          expectedRevision: tab.savedRevision,
          route: draft.route,
        });
        return saved.id;
      } catch {
        return null;
      }
    },
    [saveRequest, workspaceId, t],
  );

  return {
    activeEnvironment,
    routing,
    activeTab,
    collectionStatus,
    deleteMutation,
    duplicateMutation,
    environments,
    historyItems: historyQuery.data ?? [],
    loadedSavedRequests: savedQuery.isSuccess && savedQuery.fetchStatus === "idle" ? savedQuery.data : undefined,
    savedRequests: savedQuery.data ?? [],
    state,
    closeTab: (tabId: string) =>
      useApiRequestTabStore.getState().closeTab(workspaceId, tabId),
    closeTabs: (tabIds: string[]) =>
      useApiRequestTabStore.getState().closeTabs(workspaceId, tabIds),
    newRequest,
    cancelTab,
    openHistory,
    openSaved,
    reloadTab: async (tab: ApiRequestTab) => {
      const result = await savedQuery.refetch();
      if (!result.isSuccess) return;
      const saved = result.data?.find((item) => item.id === tab.savedRequestId);
      if (saved) useApiRequestTabStore.getState().reloadSaved(workspaceId, tab.id, saved);
    },
    saveTab,
    selectTab: (tabId: string) =>
      useApiRequestTabStore.getState().setActiveTab(workspaceId, tabId),
    sendTab,
    setRequestTab: (tabId: string, requestTab: RequestParamsTab) =>
      useApiRequestTabStore.getState().setRequestPanel(workspaceId, tabId, requestTab),
    setResponseTab: (tabId: string, responseTab: ResponseTab) =>
      useApiRequestTabStore.getState().setResponsePanel(workspaceId, tabId, responseTab),
    setSplitDirection: (direction: ApiSplitDirection) =>
      useApiRequestTabStore.getState().setApiSplitDirection(workspaceId, direction),
    setCollectionStatus,
    updateDraft,
  };
}

export function tabToInput(
  tab: ApiRequestTab,
  workspaceId: string,
  options: {
    auth?: ApiAuthConfig;
    purpose?: "save" | "send";
  } = {},
): ApiRequestInput {
  const purpose = options.purpose ?? "send";
  const body = bodyFieldsToInput(tab.draft, purpose);
  const headers =
    purpose === "save"
      ? tab.draft.headers
      : applyGeneratedHeaders(tab.draft);
  const query = purpose === "save" ? tab.draft.query : sendableKeyValues(tab.draft.query);
  return {
    workspaceId,
    name: tab.draft.name.trim() || undefined,
    parentFolderId: tab.draft.parentFolderId,
    collectionId: tab.draft.collectionId,
    authJson: JSON.stringify(options.auth ?? tab.draft.auth),
    method: tab.draft.method,
    url: stripUrlQuery(tab.draft.url),
    headers,
    query,
    body:
      purpose === "send" &&
      (tab.draft.method === "GET" || tab.draft.method === "HEAD")
        ? undefined
        : body.body,
    bodyKind: body.bodyKind,
    ...multipartRuntimeInput(tab.draft, purpose),
    preRequestScript: tab.draft.preRequestScript || null,
    postResponseScript: tab.draft.postResponseScript || null,
    scriptSchemaVersion: 1,
    temporaryVariables: purpose === "send" ? tab.draft.envVariables : [],
    timeoutMs: tab.draft.timeoutMs,
  };
}

export function validateBeforeSend(tab: ApiRequestTab, t: TFunction = createTranslator("en")): string | null {
  const replayError = historyReplayError(tab, t);
  if (replayError) return replayError;
  if (tab.draft.bodyMode === "multipart" && !["GET", "HEAD"].includes(tab.draft.method)) {
    const issue = multipartValidation(tab.draft.multipartParts);
    if (issue) return t(issue === "key" ? "api.multipart.keyError" : "api.multipart.fileError");
  }
  if (
    tab.draft.bodyMode === "raw" &&
    tab.draft.rawBodyType === "json" &&
    tab.draft.body.trim()
  ) {
    try {
      JSON.parse(tab.draft.body);
    } catch {
      return t("api.errors.invalidJsonBody");
    }
  }
  return null;
}

function applyGeneratedHeaders(
  draft: RequestDraft,
): KeyValue[] {
  let headers = sendableKeyValues(draft.headers);
  const contentType = bodyContentType(draft);
  if (contentType) headers = addHeaderIfMissing(headers, "Content-Type", contentType);

  return headers;
}

function bodyContentType(draft: RequestDraft): string | null {
  if (draft.bodyMode === "raw" && draft.rawBodyType === "json" && draft.body.trim()) return "application/json";
  if (draft.bodyMode === "form" && sendableKeyValues(draft.formBody).length) return "application/x-www-form-urlencoded";
  return null;
}
