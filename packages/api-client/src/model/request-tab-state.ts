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

export function requestTabTitle(tab: ApiRequestTab) {
  return tab.draft.name.trim() || "Untitled Request";
}

export function requestTabVisualState(tab: ApiRequestTab): ApiTabVisualState {
  if (tab.sending) {
    return "sending";
  }
  if (tab.sendError || (tab.response && tab.response.status >= 400)) {
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
  if (tab.sending) {
    return "sending";
  }
  if (tab.sendError) {
    const message = tab.sendError.toLowerCase();
    if (message.includes("timeout") || message.includes("timed out")) {
      return "timeout";
    }
    if (
      message.includes("network") ||
      message.includes("connection") ||
      message.includes("dns") ||
      message.includes("fetch")
    ) {
      return "network";
    }
    return "failed";
  }
  if (!tab.response) {
    return "idle";
  }
  if (tab.response.status >= 400) {
    return "http-error";
  }
  return tab.response.body.trim() ? "success" : "empty";
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
    query: normalizeKeyValues(draft.query),
    rawBodyType: draft.rawBodyType,
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
