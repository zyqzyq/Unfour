import type {
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
} from "@unfour/command-client";
import {
  bodyFieldsFromInput,
  defaultAuthConfig,
  historyDetailToInput,
  parseKeyValues,
  queryFromUrl,
  reconcileDraftPatch,
  savedRequestToInput,
  syncUrlQuery,
} from "../request-utils";
import type {
  ApiSplitDirection,
  ApiTabSaveState,
  ApiTabSource,
  RequestDraft,
  RequestParamsTab,
  ResponseTab,
} from "./types";

export type ApiRequestTab = {
  baseline: string | null;
  draft: RequestDraft;
  id: string;
  requestTab: RequestParamsTab;
  lastRequest: ApiRequestInput | null;
  response: ApiResponse | null;
  responseTab: ResponseTab;
  saveError: string | null;
  savedRequestId: string | null;
  sendError: string | null;
  sending: boolean;
  saving: boolean;
  source: ApiTabSource;
  sourceId: string | null;
};

export type ApiTabsState = {
  activeTabId: string | null;
  splitDirection: ApiSplitDirection;
  tabs: ApiRequestTab[];
  workspaceId: string;
};

export type ApiHistoryGroup = {
  id: string;
  items: ApiHistoryItem[];
  label: string;
};

export type ApiTabVisualState =
  | "saved"
  | "dirty"
  | "unsaved"
  | "saving"
  | "sending"
  | "success"
  | "failed";

export type ApiTabResponseState =
  | "idle"
  | "sending"
  | "success"
  | "empty"
  | "http-error"
  | "network"
  | "timeout"
  | "failed";

export const requestConfigTabs: Array<{ id: RequestParamsTab; label: string }> = [
  { id: "query", label: "Params" },
  { id: "auth", label: "Auth" },
  { id: "headers", label: "Headers" },
  { id: "body", label: "Body" },
];

export function methodBadgeLabel(method: string) {
  const normalized = method.trim().toUpperCase();
  return normalized === "DELETE" ? "DEL" : normalized;
}

export function methodToneClass(method: string) {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "text-[color:var(--u-color-info-text)]";
    case "POST":
      return "text-[color:var(--u-color-success)]";
    case "PUT":
      return "text-[color:var(--u-color-warning-text)]";
    case "PATCH":
      return "text-[color:var(--u-color-primary)]";
    case "DELETE":
      return "text-[color:var(--u-color-danger)]";
    case "HEAD":
      return "text-[color:var(--u-color-secondary-text)]";
    case "OPTIONS":
      return "text-[color:var(--u-color-neutral-text)]";
    default:
      return "text-[color:var(--u-color-text-muted)]";
  }
}

export function methodBadgeToneClass(method: string) {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "bg-[var(--u-color-info-soft)] text-[color:var(--u-color-info-text)]";
    case "POST":
      return "bg-[var(--u-color-success-soft)] text-[color:var(--u-color-success)]";
    case "PUT":
      return "bg-[var(--u-color-warning-soft)] text-[color:var(--u-color-warning-text)]";
    case "PATCH":
      return "bg-[var(--u-color-primary-soft)] text-[color:var(--u-color-primary)]";
    case "DELETE":
      return "bg-[var(--u-color-danger-soft)] text-[color:var(--u-color-danger-text)]";
    case "HEAD":
      return "bg-[var(--u-color-secondary-soft)] text-[color:var(--u-color-secondary-text)]";
    case "OPTIONS":
      return "bg-[var(--u-color-neutral-soft)] text-[color:var(--u-color-neutral-text)]";
    default:
      return "bg-[var(--u-color-surface-muted)] text-[color:var(--u-color-text-muted)]";
  }
}

export function emptyApiTabsState(workspaceId: string): ApiTabsState {
  return {
    activeTabId: null,
    splitDirection: "vertical",
    tabs: [],
    workspaceId,
  };
}

export function createNewRequestTab(
  state: ApiTabsState,
  id: string,
): ApiTabsState {
  const tab: ApiRequestTab = {
    baseline: null,
    draft: emptyDraft(),
    id,
    requestTab: "query",
    lastRequest: null,
    response: null,
    responseTab: "body",
    saveError: null,
    savedRequestId: null,
    sendError: null,
    sending: false,
    saving: false,
    source: "new",
    sourceId: null,
  };
  return {
    ...state,
    activeTabId: id,
    tabs: [...state.tabs, tab],
  };
}

export function openSavedRequest(
  state: ApiTabsState,
  saved: ApiSavedRequest,
): ApiTabsState {
  const id = `saved:${saved.id}`;
  if (state.tabs.some((tab) => tab.id === id)) {
    return { ...state, activeTabId: id };
  }

  const draft = inputToDraft(savedRequestToInput(saved, state.workspaceId));
  return {
    ...state,
    activeTabId: id,
    tabs: [
      ...state.tabs,
      {
        baseline: normalizeRequestDraft(draft),
        draft,
        id,
        requestTab: "query",
        lastRequest: null,
        response: null,
        responseTab: "body",
        saveError: null,
        savedRequestId: saved.id,
        sendError: null,
        sending: false,
        saving: false,
        source: "saved",
        sourceId: saved.id,
      },
    ],
  };
}

export function openHistoryRequest(
  state: ApiTabsState,
  history: ApiHistoryDetail,
): ApiTabsState {
  const id = `history:${history.id}`;
  const request = historyDetailToInput(history);
  if (state.tabs.some((tab) => tab.id === id)) {
    return { ...state, activeTabId: id };
  }

  return {
    ...state,
    activeTabId: id,
    tabs: [
      ...state.tabs,
      {
        baseline: null,
        draft: inputToDraft(request),
        id,
        requestTab: "query",
        lastRequest: request,
        response: historyResponse(history),
        responseTab: "body",
        saveError: null,
        savedRequestId: null,
        sendError: null,
        sending: false,
        saving: false,
        source: "history",
        sourceId: history.id,
      },
    ],
  };
}

export function updateTabDraft(
  state: ApiTabsState,
  tabId: string,
  patch: Partial<RequestDraft>,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    draft: reconcileDraftPatch(tab.draft, patch),
    saveError: null,
  }));
}

export function setActiveApiTab(
  state: ApiTabsState,
  tabId: string,
): ApiTabsState {
  return state.tabs.some((tab) => tab.id === tabId)
    ? { ...state, activeTabId: tabId }
    : state;
}

export function setTabRequestPanel(
  state: ApiTabsState,
  tabId: string,
  requestTab: RequestParamsTab,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({ ...tab, requestTab }));
}

export function setTabResponsePanel(
  state: ApiTabsState,
  tabId: string,
  responseTab: ResponseTab,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({ ...tab, responseTab }));
}

export function startTabSend(
  state: ApiTabsState,
  tabId: string,
  request: ApiRequestInput | null = null,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    lastRequest: request ?? tab.lastRequest,
    response: null,
    sendError: null,
    sending: true,
  }));
}

export function completeTabSend(
  state: ApiTabsState,
  tabId: string,
  response: ApiResponse,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    response,
    sendError: null,
    sending: false,
  }));
}

export function failTabSend(
  state: ApiTabsState,
  tabId: string,
  error: string,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    response: null,
    sendError: error,
    sending: false,
  }));
}

export function startTabSave(
  state: ApiTabsState,
  tabId: string,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    saveError: null,
    saving: true,
  }));
}

export function completeTabSave(
  state: ApiTabsState,
  tabId: string,
  saved: ApiSavedRequest,
): ApiTabsState {
  const nextId = `saved:${saved.id}`;
  const savedDraft = inputToDraft(savedRequestToInput(saved, state.workspaceId));
  const existingIndex = state.tabs.findIndex(
    (tab) => tab.id === nextId && tab.id !== tabId,
  );
  const tabs = state.tabs
    .filter((_, index) => index !== existingIndex)
    .map((tab) =>
      tab.id === tabId
        ? {
            ...tab,
            baseline: normalizeRequestDraft(savedDraft),
            draft: savedDraft,
            id: nextId,
            saveError: null,
            savedRequestId: saved.id,
            saving: false,
            source: "saved" as const,
            sourceId: saved.id,
          }
        : tab,
    );

  return {
    ...state,
    activeTabId: state.activeTabId === tabId ? nextId : state.activeTabId,
    tabs,
  };
}

export function failTabSave(
  state: ApiTabsState,
  tabId: string,
  error: string,
): ApiTabsState {
  return updateTab(state, tabId, (tab) => ({
    ...tab,
    saveError: error,
    saving: false,
  }));
}

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

export function closeApiTab(
  state: ApiTabsState,
  tabId: string,
): ApiTabsState {
  const index = state.tabs.findIndex((tab) => tab.id === tabId);
  if (index < 0) {
    return state;
  }
  const tabs = state.tabs.filter((tab) => tab.id !== tabId);
  if (state.activeTabId !== tabId) {
    return { ...state, tabs };
  }
  return {
    ...state,
    activeTabId: tabs[Math.min(index, tabs.length - 1)]?.id ?? null,
    tabs,
  };
}

export function closeApiTabs(
  state: ApiTabsState,
  tabIds: string[],
): ApiTabsState {
  const closingIds = new Set(tabIds);
  if (!closingIds.size) {
    return state;
  }
  const firstClosedIndex = state.tabs.findIndex((tab) => closingIds.has(tab.id));
  if (firstClosedIndex < 0) {
    return state;
  }
  const tabs = state.tabs.filter((tab) => !closingIds.has(tab.id));
  if (!state.activeTabId || !closingIds.has(state.activeTabId)) {
    return { ...state, tabs };
  }
  return {
    ...state,
    activeTabId: tabs[Math.min(firstClosedIndex, tabs.length - 1)]?.id ?? null,
    tabs,
  };
}

export function setApiSplitDirection(
  state: ApiTabsState,
  splitDirection: ApiSplitDirection,
): ApiTabsState {
  return { ...state, splitDirection };
}

export function normalizeRequestDraft(draft: RequestDraft): string {
  return JSON.stringify({
    auth: draft.auth,
    body: draft.body,
    bodyMode: draft.bodyMode,
    collectionId: draft.collectionId,
    folderPath: draft.folderPath.trim() || null,
    formBody: normalizeKeyValues(draft.formBody),
    headers: normalizeKeyValues(draft.headers),
    method: draft.method.toUpperCase(),
    name: draft.name.trim(),
    query: normalizeKeyValues(draft.query),
    rawBodyType: draft.rawBodyType,
    url: draft.url.trim(),
  });
}

export function groupApiHistory(
  items: ApiHistoryItem[],
  now = new Date(),
): ApiHistoryGroup[] {
  const groups = new Map<string, ApiHistoryGroup>();
  for (const item of items) {
    const date = new Date(item.createdAt);
    const days = calendarDayDifference(now, date);
    const label =
      days === 0
        ? "Today"
        : days === 1
          ? "Yesterday"
          : days > 1 && days < 7
            ? "Previous 7 Days"
            : localDateKey(date);
    const id =
      label === "Today"
        ? "history:today"
        : label === "Yesterday"
          ? "history:yesterday"
          : label === "Previous 7 Days"
            ? "history:previous-7-days"
            : `history:${label}`;
    const group = groups.get(id) ?? { id, items: [], label };
    group.items.push(item);
    groups.set(id, group);
  }
  return Array.from(groups.values());
}

function updateTab(
  state: ApiTabsState,
  tabId: string,
  update: (tab: ApiRequestTab) => ApiRequestTab,
): ApiTabsState {
  return {
    ...state,
    tabs: state.tabs.map((tab) => (tab.id === tabId ? update(tab) : tab)),
  };
}

function emptyDraft(): RequestDraft {
  return {
    auth: defaultAuthConfig(),
    body: "",
    bodyMode: "none",
    collectionId: null,
    envVariables: [],
    folderPath: "",
    formBody: [],
    headers: [],
    method: "GET",
    name: "Untitled Request",
    query: [],
    rawBodyType: "json",
    url: "",
  };
}

function inputToDraft(input: ReturnType<typeof savedRequestToInput>): RequestDraft {
  const query = input.query.length ? input.query : queryFromUrl(input.url);
  const bodyFields = bodyFieldsFromInput(input.bodyKind, input.body);
  return {
    auth: defaultAuthConfig(),
    ...bodyFields,
    collectionId: input.collectionId ?? null,
    envVariables: [],
    folderPath: input.folderPath ?? "",
    headers: input.headers,
    method: input.method,
    name: input.name ?? `${input.method} ${input.url}`,
    query,
    url: syncUrlQuery(input.url, query),
  };
}

function historyResponse(history: ApiHistoryDetail): ApiResponse | null {
  if (history.status === null) {
    return null;
  }
  return {
    historyId: history.id,
    status: history.status,
    statusText: "",
    headers: parseKeyValues(history.responseHeadersJson),
    body: history.responseBodyPreview ?? "",
    durationMs: history.durationMs ?? 0,
  };
}

function normalizeKeyValues(items: RequestDraft["headers"]) {
  return items.map((item) => ({
    enabled: item.enabled,
    key: item.key,
    value: item.value,
  }));
}

function calendarDayDifference(left: Date, right: Date) {
  const leftDay = new Date(left.getFullYear(), left.getMonth(), left.getDate());
  const rightDay = new Date(right.getFullYear(), right.getMonth(), right.getDate());
  return Math.floor((leftDay.getTime() - rightDay.getTime()) / 86_400_000);
}

function localDateKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
