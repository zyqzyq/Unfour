import type { ApiHistoryItem, ApiRequestInput, ApiResponse } from "@unfour/command-client";

import type {
  ApiSplitDirection,
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
