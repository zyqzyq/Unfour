import { redactHeaders, redactJsonBody, resolveInput } from "./helpers";
import {
  assertMockCollection,
  assertMockFolder,
  firstOrCreateMockCollectionId,
  mockActiveEnvVariables,
  mockState,
  mockStore,
  nextMockRequestSortOrder,
  normalizeMockId,
} from "./state";
import { UNHANDLED } from "./types";
import type {
  ApiClientPreferences,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
  RequestExecutionResult,
  ScriptExecutionResult,
} from "../../types";

const activeApiExecutions = new Map<string, AbortController>();
const API_TIMEOUT_STORAGE_KEY = "unfour.api.requestTimeoutMs";

export async function handleApiRequestMock<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | typeof UNHANDLED> {
  if (command === "api_client_preferences_get") {
    return readApiClientPreferences() as T;
  }

  if (command === "api_client_preferences_update") {
    const preferences = args?.preferences as ApiClientPreferences;
    validateTimeout(preferences?.requestTimeoutMs);
    writeApiClientPreferences(preferences);
    return preferences as T;
  }

  if (command === "api_cancel_request") {
    const executionId = String(args?.executionId ?? "");
    const controller = activeApiExecutions.get(executionId);
    controller?.abort("API_CANCELLED");
    return Boolean(controller) as T;
  }

  if (command === "api_history_list") {
    return mockStore.history as T;
  }

  if (command === "api_history_detail") {
    const workspaceId = String(args?.workspaceId ?? "");
    const historyId = String(args?.historyId ?? "");
    const detail = mockStore.historyDetails.find(
      (item) => item.workspaceId === workspaceId && item.id === historyId,
    );
    if (!detail) throw new Error("api history not found");
    return detail as T;
  }

  if (command === "api_saved_requests") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockStore.savedRequests.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "api_request_save") {
    const input = args?.input as ApiRequestInput;
    const collectionId = input.collectionId ?? firstOrCreateMockCollectionId(input.workspaceId);
    const parentFolderId = normalizeMockId(input.parentFolderId);
    assertMockCollection(input.workspaceId, collectionId);
    assertMockFolder(input.workspaceId, collectionId, parentFolderId);
    const saved: ApiSavedRequest = {
      id: crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name || `${input.method} ${input.url}`,
      collectionId,
      parentFolderId,
      sortOrder: nextMockRequestSortOrder(input.workspaceId, collectionId, parentFolderId),
      authJson: input.authJson ?? JSON.stringify({ type: "none" }),
      method: input.method,
      url: input.url,
      headersJson: JSON.stringify(redactHeaders(input.headers)),
      queryJson: JSON.stringify(input.query),
      body: redactJsonBody(input.body),
      bodyKind: input.bodyKind,
      settingsJson: requestSettingsJson(input.timeoutMs),
      preRequestScript: input.preRequestScript ?? null,
      postResponseScript: input.postResponseScript ?? null,
      scriptSchemaVersion: input.scriptSchemaVersion ?? 1,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      deletedAt: null,
      revision: 1,
      syncStatus: "local",
      remoteId: null,
    };
    mockStore.savedRequests = [saved, ...mockStore.savedRequests];
    return saved as T;
  }

  if (command === "api_request_update") {
    const input = args?.input as ApiRequestInput;
    const workspaceId = String(args?.workspaceId ?? input.workspaceId);
    const requestId = String(args?.requestId ?? "");
    if (workspaceId !== input.workspaceId) throw new Error("api request workspace mismatch");
    const collectionId = input.collectionId ?? firstOrCreateMockCollectionId(workspaceId);
    const parentFolderId = normalizeMockId(input.parentFolderId);
    assertMockCollection(workspaceId, collectionId);
    assertMockFolder(workspaceId, collectionId, parentFolderId);
    const index = mockStore.savedRequests.findIndex(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (index === -1) throw new Error("api request not found");
    const current = mockStore.savedRequests[index];
    const saved: ApiSavedRequest = {
      ...current,
      name: input.name || `${input.method} ${input.url}`,
      collectionId,
      parentFolderId,
      sortOrder:
        current.collectionId === collectionId && current.parentFolderId === parentFolderId
          ? current.sortOrder
          : nextMockRequestSortOrder(workspaceId, collectionId, parentFolderId),
      authJson: input.authJson ?? JSON.stringify({ type: "none" }),
      method: input.method,
      url: input.url,
      headersJson: JSON.stringify(redactHeaders(input.headers)),
      queryJson: JSON.stringify(input.query),
      body: redactJsonBody(input.body),
      bodyKind: input.bodyKind,
      settingsJson: requestSettingsJson(input.timeoutMs),
      preRequestScript: input.preRequestScript ?? null,
      postResponseScript: input.postResponseScript ?? null,
      scriptSchemaVersion: input.scriptSchemaVersion ?? 1,
      updatedAt: new Date().toISOString(),
      revision: current.revision + 1,
      syncStatus: "pending",
    };
    mockStore.savedRequests = [
      ...mockStore.savedRequests.slice(0, index),
      saved,
      ...mockStore.savedRequests.slice(index + 1),
    ];
    return saved as T;
  }

  if (command === "api_request_duplicate") {
    const workspaceId = String(args?.workspaceId ?? "");
    const requestId = String(args?.requestId ?? "");
    const source = mockStore.savedRequests.find(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (!source) throw new Error("api request not found");
    const now = new Date().toISOString();
    const duplicate: ApiSavedRequest = {
      ...source,
      id: crypto.randomUUID(),
      name: `${source.name} Copy`,
      createdAt: now,
      updatedAt: now,
      revision: 1,
      syncStatus: "local",
      remoteId: null,
    };
    mockStore.savedRequests = [duplicate, ...mockStore.savedRequests];
    return duplicate as T;
  }

  if (command === "api_request_delete") {
    const workspaceId = String(args?.workspaceId ?? "");
    const requestId = String(args?.requestId ?? "");
    const initialLength = mockStore.savedRequests.length;
    mockStore.savedRequests = mockStore.savedRequests.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === requestId),
    );
    if (mockStore.savedRequests.length === initialLength) {
      throw new Error("api request not found");
    }
    return mockStore.savedRequests.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "api_send_request_v2" || command === "api_send_request") {
    const input = args?.input as ApiRequestInput;
    const versioned = command === "api_send_request_v2";
    const executionId = String(args?.executionId ?? crypto.randomUUID());
    if (activeApiExecutions.has(executionId)) {
      throw { code: "VALIDATION_ERROR", message: "API execution id is already active" };
    }
    const controller = new AbortController();
    activeApiExecutions.set(executionId, controller);
    const preRequest = input.preRequestScript?.trim()
      ? unsupportedBrowserScript()
      : skippedScript();
    if (versioned && preRequest.status === "failed") {
      activeApiExecutions.delete(executionId);
      const execution: RequestExecutionResult = {
        response: null,
        httpError: null,
        httpErrorCode: null,
        preRequest,
        postResponse: skippedScript(),
      };
      return execution as T;
    }
    const started = performance.now();
    const timeoutMs = input.timeoutMs ?? readApiClientPreferences().requestTimeoutMs;
    validateTimeout(timeoutMs);
    const timeoutHandle = timeoutMs > 0
      ? globalThis.setTimeout(() => controller.abort("API_TIMEOUT"), timeoutMs)
      : null;
    try {
      const resolved = resolveInput(input, mockActiveEnvVariables(input.workspaceId));
      const url = new URL(resolved.url);
      resolved.query
        .filter((item) => item.enabled && item.key)
        .forEach((item) => url.searchParams.append(item.key, item.value));
      const headers = Object.fromEntries(
        resolved.headers
          .filter((item) => item.enabled && item.key)
          .map((item) => [item.key, item.value]),
      );
      const response = await fetch(url, {
        method: resolved.method,
        headers,
        body:
          resolved.method === "GET" || resolved.method === "HEAD"
            ? undefined
            : resolved.body || undefined,
        signal: controller.signal,
      });
      const body = await response.text();
      const result: ApiResponse = {
        historyId: crypto.randomUUID(),
        status: response.status,
        statusText: response.statusText,
        headers: Array.from(response.headers.entries()).map(([key, value]) => ({
          key,
          value,
          enabled: true,
        })),
        body,
        durationMs: Math.round(performance.now() - started),
      };
      mockStore.history = [
        {
          id: result.historyId,
          workspaceId: input.workspaceId,
          name: input.name ?? null,
          method: resolved.method,
          url: resolved.url,
          status: result.status,
          durationMs: result.durationMs,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          deletedAt: null,
          revision: 1,
          syncStatus: "local",
          remoteId: null,
        },
        ...mockStore.history,
      ];
      mockStore.historyDetails = [
        {
          id: result.historyId,
          workspaceId: input.workspaceId,
          name: input.name ?? null,
          method: resolved.method,
          url: resolved.url,
          requestHeadersJson: JSON.stringify(redactHeaders(input.headers)),
          requestQueryJson: JSON.stringify(input.query),
          requestBody: redactJsonBody(input.body),
          status: result.status,
          durationMs: result.durationMs,
          responseHeadersJson: JSON.stringify(result.headers),
          responseBodyPreview: body.slice(0, 20_000),
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          deletedAt: null,
          revision: 1,
          syncStatus: "local",
          remoteId: null,
        },
        ...mockStore.historyDetails,
      ];
      if (!versioned) return result as T;
      const execution: RequestExecutionResult = {
        response: result,
        httpError: null,
        httpErrorCode: null,
        preRequest,
        postResponse: input.postResponseScript?.trim()
          ? unsupportedBrowserScript()
          : skippedScript(),
      };
      return execution as T;
    } catch (error) {
      if (!versioned) throw error;
      const code = controller.signal.reason === "API_TIMEOUT"
        ? "API_TIMEOUT"
        : controller.signal.reason === "API_CANCELLED"
          ? "API_CANCELLED"
          : "NETWORK_ERROR";
      return {
        response: null,
        httpError: code === "API_TIMEOUT"
          ? "API request timed out"
          : code === "API_CANCELLED"
            ? "API request cancelled"
            : error instanceof Error ? error.message : String(error),
        httpErrorCode: code,
        preRequest,
        postResponse: skippedScript(),
      } as T;
    } finally {
      if (timeoutHandle !== null) globalThis.clearTimeout(timeoutHandle);
      if (activeApiExecutions.get(executionId) === controller) {
        activeApiExecutions.delete(executionId);
      }
    }
  }

  return UNHANDLED;
}

function requestSettingsJson(timeoutMs: number | null | undefined) {
  return JSON.stringify({ timeoutMs: timeoutMs ?? null });
}

function readApiClientPreferences(): ApiClientPreferences {
  try {
    const value = globalThis.localStorage?.getItem(API_TIMEOUT_STORAGE_KEY);
    const requestTimeoutMs = value === null || value === undefined ? 0 : Number(value);
    validateTimeout(requestTimeoutMs);
    return { requestTimeoutMs };
  } catch {
    return { requestTimeoutMs: 0 };
  }
}

function writeApiClientPreferences(preferences: ApiClientPreferences) {
  globalThis.localStorage?.setItem(
    API_TIMEOUT_STORAGE_KEY,
    String(preferences.requestTimeoutMs),
  );
}

function validateTimeout(value: number | undefined): asserts value is number {
  if (!Number.isSafeInteger(value) || (value ?? -1) < 0) {
    throw { code: "VALIDATION_ERROR", message: "Request timeout must be a non-negative integer" };
  }
}

function skippedScript(): ScriptExecutionResult {
  return {
    status: "skipped",
    durationMs: 0,
    console: [],
    tests: [],
    error: null,
  };
}

function unsupportedBrowserScript(): ScriptExecutionResult {
  return {
    status: "failed",
    durationMs: 0,
    console: [],
    tests: [],
    error: {
      kind: "validation",
      code: "SCRIPT_DESKTOP_REQUIRED",
      message: "Request scripts require the desktop runtime.",
    },
  };
}
