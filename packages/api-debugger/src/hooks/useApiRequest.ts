import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteApiRequest,
  duplicateApiRequest,
  getWorkspaceEnvironment,
  listSavedApiRequests,
  saveApiRequest,
  sendApiRequest,
  updateWorkspaceEnvironment,
  type ApiHistoryDetail,
  type ApiRequestInput,
  type ApiResponse,
  type ApiSavedRequest,
  type KeyValue,
} from "@unfour/command-client";
import {
  historyDetailToInput,
  parseCollectionImport,
  parseKeyValues,
  savedRequestToInput,
} from "../request-utils";
import { deriveApiRequestState, formatError } from "../model/api-request-state";
import type { ApiRequestState } from "../model/types";

export const methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];

export function useApiRequest({
  selectedRequestId,
  setSelectedRequestId,
  workspaceId,
}: {
  selectedRequestId: string | null;
  setSelectedRequestId: (requestId: string | null) => void;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();
  const importInputRef = useRef<HTMLInputElement>(null);
  const [method, setMethod] = useState("GET");
  const [url, setUrl] = useState("{{base_url}}/get");
  const [name, setName] = useState("Health check");
  const [folderPath, setFolderPath] = useState("Examples");
  const [headers, setHeaders] = useState<KeyValue[]>([
    { key: "Accept", value: "application/json", enabled: true },
  ]);
  const [query, setQuery] = useState<KeyValue[]>([
    { key: "source", value: "{{source}}", enabled: true },
  ]);
  const [body, setBody] = useState("{\n  \"hello\": \"workspace\"\n}");
  const [envVariables, setEnvVariables] = useState<KeyValue[]>([]);
  const [collectionStatus, setCollectionStatus] = useState("");
  const [loadedSavedRequestId, setLoadedSavedRequestId] = useState<string | null>(null);
  const [response, setResponse] = useState<ApiResponse | null>(null);

  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const environmentQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["workspace-environment", workspaceId],
    queryFn: () => getWorkspaceEnvironment(workspaceId),
  });

  useEffect(() => {
    if (environmentQuery.data) {
      setEnvVariables(environmentQuery.data.variables);
    }
  }, [environmentQuery.data]);

  useEffect(() => {
    if (!savedQuery.data || !selectedRequestId) {
      return;
    }

    const selected = savedQuery.data.find((item) => item.id === selectedRequestId);
    if (!selected) {
      setSelectedRequestId(null);
      setLoadedSavedRequestId(null);
      return;
    }

    if (loadedSavedRequestId !== selected.id) {
      loadSavedRequest(selected);
    }
  }, [loadedSavedRequestId, savedQuery.data, selectedRequestId, setSelectedRequestId]);

  const input = useMemo<ApiRequestInput>(
    () => ({
      workspaceId,
      name,
      folderPath,
      method,
      url,
      headers,
      query,
      body: method === "GET" || method === "HEAD" ? undefined : body,
      bodyKind: "json",
      timeoutMs: 60_000,
    }),
    [body, folderPath, headers, method, name, query, url, workspaceId],
  );

  const sendMutation = useMutation({
    mutationFn: sendApiRequest,
    onSuccess: (result) => {
      setResponse(result);
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
    },
  });

  const saveMutation = useMutation({
    mutationFn: saveApiRequest,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const duplicateSavedMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const deleteSavedMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const saveEnvironmentMutation = useMutation({
    mutationFn: (variables: KeyValue[]) => updateWorkspaceEnvironment(workspaceId, variables),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["workspace-environment", workspaceId] }),
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

  function submit(event: FormEvent) {
    event.preventDefault();
    sendMutation.mutate(input);
  }

  function loadRequestDraft(request: ApiRequestInput) {
    setName(request.name ?? `${request.method} ${request.url}`);
    setFolderPath(request.folderPath ?? "");
    setMethod(request.method);
    setUrl(request.url);
    setHeaders(request.headers);
    setQuery(request.query);
    setBody(request.body ?? "");
  }

  function loadSavedRequest(saved: ApiSavedRequest) {
    setSelectedRequestId(saved.id);
    setLoadedSavedRequestId(saved.id);
    loadRequestDraft(savedRequestToInput(saved, workspaceId));
  }

  function loadHistoryRequest(history: ApiHistoryDetail) {
    setSelectedRequestId(null);
    setLoadedSavedRequestId(null);
    loadRequestDraft(historyDetailToInput(history));
  }

  function newRequest() {
    setSelectedRequestId(null);
    setLoadedSavedRequestId(null);
    setResponse(null);
    setName("Untitled request");
    setFolderPath("");
    setMethod("GET");
    setUrl("");
    setHeaders([]);
    setQuery([]);
    setBody("");
  }

  function deleteSelectedRequest() {
    if (!selectedRequestId) {
      return;
    }
    const requestId = selectedRequestId;
    setSelectedRequestId(null);
    setLoadedSavedRequestId(null);
    deleteSavedMutation.mutate(requestId);
  }

  function duplicateSelectedRequest() {
    if (selectedRequestId) {
      duplicateSavedMutation.mutate(selectedRequestId);
    }
  }

  function saveEnvironment() {
    saveEnvironmentMutation.mutate(envVariables);
  }

  function exportCollection() {
    const payload = {
      version: 1,
      exportedAt: new Date().toISOString(),
      workspaceId,
      savedRequests: (savedQuery.data ?? []).map((item) => ({
        name: item.name,
        folderPath: item.folderPath,
        method: item.method,
        url: item.url,
        headers: parseKeyValues(item.headersJson),
        query: parseKeyValues(item.queryJson),
        body: item.body,
        bodyKind: item.bodyKind,
      })),
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json;charset=utf-8",
    });
    const href = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = href;
    link.download = `unfour-api-collection-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(href);
    setCollectionStatus(
      `Exported ${payload.savedRequests.length} request${payload.savedRequests.length === 1 ? "" : "s"}`,
    );
  }

  async function importCollection(file: File | undefined) {
    if (!file) {
      return;
    }
    try {
      const parsed = JSON.parse(await file.text());
      const requests = parseCollectionImport(parsed, workspaceId);
      if (!requests.length) {
        setCollectionStatus("No importable requests found");
        return;
      }
      importCollectionMutation.mutate(requests);
    } catch (error) {
      setCollectionStatus(formatError(error));
    }
  }

  const savedRequests = savedQuery.data ?? [];
  const selectedSavedRequest =
    savedRequests.find((item) => item.id === selectedRequestId) ?? null;
  const requestState: ApiRequestState = deriveApiRequestState({
    error: sendMutation.error,
    hasSelectedRequest: Boolean(selectedSavedRequest),
    isSending: sendMutation.isPending,
    response,
  });

  return {
    body,
    collectionStatus,
    deleteSavedMutation,
    deleteSelectedRequest,
    duplicateSavedMutation,
    duplicateSelectedRequest,
    envVariables,
    exportCollection,
    folderPath,
    headers,
    importCollection,
    importCollectionMutation,
    importInputRef,
    input,
    loadHistoryRequest,
    method,
    name,
    newRequest,
    query,
    requestState,
    response,
    saveEnvironment,
    saveEnvironmentMutation,
    saveMutation,
    savedQuery,
    savedRequests,
    selectedSavedRequest,
    sendMutation,
    setBody,
    setEnvVariables,
    setFolderPath,
    setHeaders,
    setMethod,
    setName,
    setQuery,
    setResponse,
    setUrl,
    submit,
    url,
  };
}
