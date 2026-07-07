import { useCallback, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  activateApiEnvironment,
  deleteApiRequest,
  duplicateApiRequest,
  getApiHistoryDetail,
  listApiEnvironments,
  listApiHistory,
  listSavedApiRequests,
  saveApiRequest,
  sendApiRequest,
  updateApiRequest,
  type ApiRequestInput,
  type KeyValue,
} from "@unfour/command-client";
import { formatError } from "../model/api-request-state";
import {
  DEFAULT_API_TAB_STATE,
  useApiRequestTabStore,
} from "../model/api-request-tab-state";
import type { ApiRequestTab } from "../model/request-tabs";
import {
  addHeaderIfMissing,
  addQueryIfMissing,
  bodyFieldsToInput,
  hasHeader,
  resolveTemplateLoose,
  sendableKeyValues,
  stripUrlQuery,
} from "../request-utils";
import type {
  ApiSplitDirection,
  RequestDraft,
  RequestParamsTab,
  ResponseTab,
} from "../model/types";

export function useApiRequestTabs(workspaceId: string) {
  const queryClient = useQueryClient();
  const importInputRef = useRef<HTMLInputElement>(null);
  const [collectionStatus, setCollectionStatus] = useState("");
  const state = useApiRequestTabStore(
    (s) => s.byWorkspace[workspaceId] ?? DEFAULT_API_TAB_STATE,
  );

  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });
  const environmentsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-environments", workspaceId],
    queryFn: () => listApiEnvironments(workspaceId),
  });

  const sendMutation = useMutation({
    mutationFn: ({ input }: { input: ApiRequestInput; tabId: string }) =>
      sendApiRequest(input),
    onSuccess: (response, variables) => {
      useApiRequestTabStore.getState().completeTabSend(workspaceId, variables.tabId, response);
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
    },
    onError: (error, variables) =>
      useApiRequestTabStore.getState().failTabSend(workspaceId, variables.tabId, formatError(error)),
  });

  const saveMutation = useMutation({
    mutationFn: ({
      input,
      requestId,
    }: {
      input: ApiRequestInput;
      requestId?: string | null;
      tabId: string;
    }) =>
      requestId
        ? updateApiRequest(input.workspaceId, requestId, input)
        : saveApiRequest(input),
    onSuccess: (saved, variables) => {
      useApiRequestTabStore.getState().completeTabSave(workspaceId, variables.tabId, saved);
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
      // Also invalidate collections in case a default was auto-created
      queryClient.invalidateQueries({ queryKey: ["api-collections", workspaceId] });
    },
    onError: (error, variables) =>
      useApiRequestTabStore.getState().failTabSave(workspaceId, variables.tabId, formatError(error)),
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
  const activateEnvironmentMutation = useMutation({
    mutationFn: (environmentId: string | null) =>
      activateApiEnvironment(workspaceId, environmentId),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["api-environments", workspaceId],
      }),
  });
  const importCollectionMutation = useMutation({
    mutationFn: async (requests: ApiRequestInput[]) => {
      for (const request of requests) {
        await saveApiRequest({ ...request, workspaceId });
      }
      return requests.length;
    },
    onSuccess: (count) => {
      setCollectionStatus(`Imported ${count} request${count === 1 ? "" : "s"}`);
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
    },
    onError: (error) => setCollectionStatus(formatError(error)),
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
  const envVariables = useMemo(
    () => activeEnvironment?.variables ?? [],
    [activeEnvironment],
  );

  const newRequest = useCallback(() => {
    useApiRequestTabStore.getState().newRequest(workspaceId);
  }, [workspaceId]);

  const openSaved = useCallback(
    (requestId: string) => {
      const saved = savedQuery.data?.find((item) => item.id === requestId);
      if (saved) {
        useApiRequestTabStore.getState().openSaved(workspaceId, saved);
      }
    },
    [savedQuery.data, workspaceId],
  );

  const openHistory = useCallback(
    async (historyId: string) => {
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
      const validationError = validateBeforeSend(tab);
      if (validationError) {
        useApiRequestTabStore.getState().failTabSend(workspaceId, tab.id, validationError);
        return;
      }
      const input = tabToInput(tab, workspaceId, {
        envVariables,
        purpose: "send",
      });
      useApiRequestTabStore.getState().startTabSend(workspaceId, tab.id, input);
      sendRequest({
        input,
        tabId: tab.id,
      });
    },
    [envVariables, sendRequest, workspaceId],
  );

  const saveTab = useCallback(
    async (
      tab: ApiRequestTab,
      identity?: {
        collectionId: string | null;
        name: string;
        parentFolderId: string | null;
      },
    ) => {
      const draft = identity ? { ...tab.draft, ...identity } : tab.draft;
      if (identity) {
        useApiRequestTabStore.getState().updateTabDraft(workspaceId, tab.id, identity);
      }
      useApiRequestTabStore.getState().startTabSave(workspaceId, tab.id);
      try {
        const saved = await saveRequest({
          input: tabToInput({ ...tab, draft }, workspaceId, {
            envVariables,
            purpose: "save",
          }),
          requestId: tab.savedRequestId,
          tabId: tab.id,
        });
        return saved.id;
      } catch {
        return null;
      }
    },
    [envVariables, saveRequest, workspaceId],
  );

  return {
    activeEnvironment,
    activeTab,
    collectionStatus,
    deleteMutation,
    duplicateMutation,
    environments,
    envVariables,
    historyItems: historyQuery.data ?? [],
    importInputRef,
    savedRequests: savedQuery.data ?? [],
    state,
    activateEnvironment: (environmentId: string | null) =>
      activateEnvironmentMutation.mutate(environmentId),
    closeTab: (tabId: string) =>
      useApiRequestTabStore.getState().closeTab(workspaceId, tabId),
    closeTabs: (tabIds: string[]) =>
      useApiRequestTabStore.getState().closeTabs(workspaceId, tabIds),
    importCollectionMutation,
    newRequest,
    openHistory,
    openSaved,
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
    envVariables?: KeyValue[];
    purpose?: "save" | "send";
  } = {},
): ApiRequestInput {
  const purpose = options.purpose ?? "send";
  const body = bodyFieldsToInput(tab.draft, purpose);
  const headers =
    purpose === "save"
      ? tab.draft.headers
      : applyGeneratedHeaders(tab.draft, options.envVariables ?? []);
  const query =
    purpose === "save"
      ? tab.draft.query
      : applyGeneratedQuery(tab.draft, options.envVariables ?? []);
  return {
    workspaceId,
    name: tab.draft.name,
    parentFolderId: tab.draft.parentFolderId,
    collectionId: tab.draft.collectionId,
    authJson: JSON.stringify(tab.draft.auth),
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
    timeoutMs: 60_000,
  };
}

function validateBeforeSend(tab: ApiRequestTab): string | null {
  if (
    tab.draft.bodyMode === "raw" &&
    tab.draft.rawBodyType === "json" &&
    tab.draft.body.trim()
  ) {
    try {
      JSON.parse(tab.draft.body);
    } catch (error) {
      return `Request body is not valid JSON: ${formatError(error)}`;
    }
  }
  return null;
}

function applyGeneratedHeaders(
  draft: RequestDraft,
  envVariables: KeyValue[],
): KeyValue[] {
  let headers = sendableKeyValues(draft.headers);
  if (draft.bodyMode === "raw" && draft.rawBodyType === "json" && draft.body.trim()) {
    headers = addHeaderIfMissing(headers, "Content-Type", "application/json");
  }
  if (draft.bodyMode === "form" && sendableKeyValues(draft.formBody).length) {
    headers = addHeaderIfMissing(
      headers,
      "Content-Type",
      "application/x-www-form-urlencoded",
    );
  }

  // Explicit Authorization in the Headers table wins over generated Auth headers.
  if (draft.auth.type === "bearer" && !hasHeader(headers, "Authorization")) {
    const token = resolveTemplateLoose(draft.auth.token, envVariables);
    if (token.trim()) {
      headers = [
        ...headers,
        {
          enabled: true,
          key: "Authorization",
          value: `Bearer ${token}`,
        },
      ];
    }
  }
  if (draft.auth.type === "basic" && !hasHeader(headers, "Authorization")) {
    const username = resolveTemplateLoose(draft.auth.username, envVariables);
    const password = resolveTemplateLoose(draft.auth.password, envVariables);
    if (username || password) {
      headers = [
        ...headers,
        {
          enabled: true,
          key: "Authorization",
          value: `Basic ${encodeBasicCredential(username, password)}`,
        },
      ];
    }
  }
  if (draft.auth.type === "api-key" && draft.auth.addTo === "header") {
    const key = resolveTemplateLoose(draft.auth.key, envVariables).trim();
    const value = resolveTemplateLoose(draft.auth.value, envVariables);
    if (key && !hasHeader(headers, key)) {
      headers = [
        ...headers,
        {
          enabled: true,
          key,
          value,
        },
      ];
    }
  }
  return headers;
}

function applyGeneratedQuery(
  draft: RequestDraft,
  envVariables: KeyValue[],
): KeyValue[] {
  let query = sendableKeyValues(draft.query);
  if (draft.auth.type === "api-key" && draft.auth.addTo === "query") {
    const key = resolveTemplateLoose(draft.auth.key, envVariables).trim();
    const value = resolveTemplateLoose(draft.auth.value, envVariables);
    if (key) {
      query = addQueryIfMissing(query, key, value);
    }
  }
  return query;
}

function encodeBasicCredential(username: string, password: string): string {
  const value = `${username}:${password}`;
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}
