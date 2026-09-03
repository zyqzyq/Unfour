import type { ApiTabSaveState, RequestDraft } from "./types";
import type {
  ApiRequestTab,
  ApiTabResponseState,
  ApiTabVisualState,
} from "./request-tab-types";

export function getTabSaveState(tab: ApiRequestTab): ApiTabSaveState {
  if (tab.saving) {
    return "saving";
  }
  if (!tab.baseline) {
    return "unsaved";
  }
  return normalizeRequestDraft(tab.draft) === tab.baseline ? "saved" : "dirty";
}

export function requestTabTitle(
  tab: ApiRequestTab,
  fallback = "Untitled Request",
) {
  return tab.draft.name.trim() || fallback;
}

export function requestTabVisualState(tab: ApiRequestTab): ApiTabVisualState {
  if (tab.cancelling) {
    return "cancelling";
  }
  if (tab.sending) {
    return "sending";
  }
  if (tab.sendErrorCode === "API_CANCELLED") {
    return "cancelled";
  }
  if (hasRequestFailure(tab)) {
    return "failed";
  }
  if (tab.response) {
    return "success";
  }
  return getTabSaveState(tab);
}

export function deriveTabResponseState(
  tab: ApiRequestTab,
): ApiTabResponseState {
  if (tab.cancelling) {
    return "cancelling";
  }
  if (tab.sending) {
    return "sending";
  }
  if (tab.sendError) {
    return classifyTabSendError(tab.sendError, tab.sendErrorCode);
  }
  if (tab.execution?.preRequest.status === "timeout") {
    return "pre-script-timeout";
  }
  if (tab.execution?.preRequest.status === "failed") {
    return "pre-script-error";
  }
  if (!tab.response) {
    return "idle";
  }
  if (tab.response.status >= 400) {
    return "http-error";
  }
  return tab.response.body.trim() ? "success" : "empty";
}

function hasRequestFailure(tab: ApiRequestTab): boolean {
  return Boolean(
    tab.sendError ||
      tab.execution?.preRequest.status === "failed" ||
      tab.execution?.preRequest.status === "timeout" ||
      tab.execution?.postResponse.status === "failed" ||
      tab.execution?.postResponse.status === "timeout" ||
      (tab.response && tab.response.status >= 400),
  );
}

function classifyTabSendError(
  error: string,
  errorCode: string | null,
): ApiTabResponseState {
  if (errorCode === "API_CANCELLED") return "cancelled";
  if (errorCode === "API_TIMEOUT") return "timeout";
  if (errorCode === "NETWORK_ERROR") return "network";
  const message = error.toLowerCase();
  if (message.includes("timeout") || message.includes("timed out")) return "timeout";
  if (["network", "connection", "dns", "fetch"].some((term) => message.includes(term))) {
    return "network";
  }
  return "failed";
}

export function normalizeRequestDraft(draft: RequestDraft): string {
  return JSON.stringify({
    auth: draft.auth,
    body: draft.body,
    bodyMode: draft.bodyMode,
    collectionId: draft.collectionId,
    formBody: normalizeKeyValues(draft.formBody),
    headers: normalizeKeyValues(draft.headers),
    method: draft.method.toUpperCase(),
    name: draft.name.trim(),
    parentFolderId: draft.parentFolderId,
    postResponseScript: draft.postResponseScript,
    preRequestScript: draft.preRequestScript,
    query: normalizeKeyValues(draft.query),
    rawBodyType: draft.rawBodyType,
    timeoutMs: draft.timeoutMs,
    url: draft.url.trim(),
  });
}

function normalizeKeyValues(items: RequestDraft["headers"]) {
  return items.map((item) => ({
    enabled: item.enabled,
    key: item.key,
    value: item.value,
  }));
}
