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
  type ApiRequestInput,
  type KeyValue,
} from "@unfour/command-client";
import { formatError } from "../model/api-request-state";
import {
  closeApiTab,
  completeTabSave,
  completeTabSend,
  createNewRequestTab,
  emptyApiTabsState,
  failTabSave,
  failTabSend,
  openHistoryRequest,
  openSavedRequest,
  setActiveApiTab,
  setApiSplitDirection,
  setTabRequestPanel,
  setTabResponsePanel,
  startTabSave,
  startTabSend,
  updateTabDraft,
  type ApiRequestTab,
} from "../model/request-tabs";
import {
  addHeaderIfMissing,
  addQueryIfMissing,
  bodyFieldsToInput,
  hasHeader,
  headersWithAuthMetadata,
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
  const nextNewId = useRef(1);
  const importInputRef = useRef<HTMLInputElement>(null);
  const [collectionStatus, setCollectionStatus] = useState("");
  const [state, setState] = useState(() =>
    createNewRequestTab(emptyApiTabsState(workspaceId), "new:1"),
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
      setState((current) => completeTabSend(current, variables.tabId, response));
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
    },
    onError: (error, variables) =>
      setState((current) =>
        failTabSend(current, variables.tabId, formatError(error)),
      ),
  });

  const saveMutation = useMutation({
    mutationFn: ({ input }: { input: ApiRequestInput; tabId: string }) =>
      saveApiRequest(input),
    onSuccess: (saved, variables) => {
      setState((current) => completeTabSave(current, variables.tabId, saved));
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
    },
    onError: (error, variables) =>
      setState((current) =>
        failTabSave(current, variables.tabId, formatError(error)),
      ),
  });

  const duplicateMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });
  const deleteMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: (_, requestId) => {
      setState((current) => closeApiTab(current, `saved:${requestId}`));
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

  const activeTab =
    state.tabs.find((tab) => tab.id === state.activeTabId) ?? null;
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
    nextNewId.current += 1;
    const id = `new:${nextNewId.current}`;
    setState((current) => createNewRequestTab(current, id));
  }, []);

  const openSaved = useCallback((requestId: string) => {
    const saved = savedQuery.data?.find((item) => item.id === requestId);
    if (saved) {
      setState((current) => openSavedRequest(current, saved));
    }
  }, [savedQuery.data]);

  const openHistory = useCallback(async (historyId: string) => {
    const detail = await getApiHistoryDetail(workspaceId, historyId);
    setState((current) => openHistoryRequest(current, detail));
  }, [workspaceId]);

  function updateDraft(tabId: string, patch: Partial<RequestDraft>) {
    setState((current) => updateTabDraft(current, tabId, patch));
  }

  const sendTab = useCallback((tab: ApiRequestTab) => {
    const validationError = validateBeforeSend(tab);
    if (validationError) {
      setState((current) => failTabSend(current, tab.id, validationError));
      return;
    }
    setState((current) => startTabSend(current, tab.id));
    sendRequest({
      input: tabToInput(tab, workspaceId, {
        envVariables,
        purpose: "send",
      }),
      tabId: tab.id,
    });
  }, [envVariables, sendRequest, workspaceId]);

  const saveTab = useCallback(async (
    tab: ApiRequestTab,
    identity?: { folderPath: string; name: string },
  ) => {
    const draft = identity ? { ...tab.draft, ...identity } : tab.draft;
    if (identity) {
      setState((current) => updateTabDraft(current, tab.id, identity));
    }
    setState((current) => startTabSave(current, tab.id));
    try {
      const saved = await saveRequest({
        input: tabToInput({ ...tab, draft }, workspaceId, {
          envVariables,
          purpose: "save",
        }),
        tabId: tab.id,
      });
      return saved.id;
    } catch {
      return null;
    }
  }, [envVariables, saveRequest, workspaceId]);

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
      setState((current) => closeApiTab(current, tabId)),
    importCollectionMutation,
    newRequest,
    openHistory,
    openSaved,
    saveTab,
    selectTab: (tabId: string) =>
      setState((current) => setActiveApiTab(current, tabId)),
    sendTab,
    setRequestTab: (tabId: string, requestTab: RequestParamsTab) =>
      setState((current) => setTabRequestPanel(current, tabId, requestTab)),
    setResponseTab: (tabId: string, responseTab: ResponseTab) =>
      setState((current) => setTabResponsePanel(current, tabId, responseTab)),
    setSplitDirection: (direction: ApiSplitDirection) =>
      setState((current) => setApiSplitDirection(current, direction)),
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
      ? headersWithAuthMetadata(tab.draft.headers, tab.draft.auth)
      : applyGeneratedHeaders(tab.draft, options.envVariables ?? []);
  const query =
    purpose === "save"
      ? tab.draft.query
      : applyGeneratedQuery(tab.draft, options.envVariables ?? []);
  return {
    workspaceId,
    name: tab.draft.name,
    folderPath: tab.draft.folderPath || null,
    method: tab.draft.method,
    url: stripUrlQuery(tab.draft.url),
    headers,
    query,
    body:
      tab.draft.method === "GET" || tab.draft.method === "HEAD"
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
  if (draft.auth.type === "bearer" && draft.auth.token.trim() && !hasHeader(headers, "Authorization")) {
    headers = [
      ...headers,
      {
        enabled: true,
        key: "Authorization",
        value: `Bearer ${draft.auth.token}`,
      },
    ];
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
  if (
    draft.auth.type === "api-key" &&
    draft.auth.addTo === "header" &&
    draft.auth.key.trim() &&
    !hasHeader(headers, draft.auth.key)
  ) {
    headers = [
      ...headers,
      {
        enabled: true,
        key: draft.auth.key.trim(),
        value: draft.auth.value,
      },
    ];
  }
  return headers;
}

function applyGeneratedQuery(
  draft: RequestDraft,
  _envVariables: KeyValue[],
): KeyValue[] {
  let query = sendableKeyValues(draft.query);
  if (
    draft.auth.type === "api-key" &&
    draft.auth.addTo === "query" &&
    draft.auth.key.trim()
  ) {
    query = addQueryIfMissing(query, draft.auth.key, draft.auth.value);
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
