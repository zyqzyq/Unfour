import type {
  ApiHistoryItem,
  ApiRequestInput,
  ApiResponse,
  RequestExecutionResult,
} from "@unfour/command-client";

import type {
  ApiSplitDirection,
  ApiTabSource,
  RequestDraft,
  RequestParamsTab,
  ResponseTab,
} from "./types";

export type ApiRequestTab = {
  baseline: string | null;
  cancelling: boolean;
  draft: RequestDraft;
  execution: RequestExecutionResult | null;
  id: string;
  executionId: string | null;
  requestTab: RequestParamsTab;
  lastRequest: ApiRequestInput | null;
  response: ApiResponse | null;
  responseTab: ResponseTab;
  saveError: string | null;
  savedRequestId: string | null;
  sendError: string | null;
  sendErrorCode: string | null;
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
  | "cancelling"
  | "cancelled"
  | "success"
  | "failed";

export type ApiTabResponseState =
  | "idle"
  | "sending"
  | "cancelling"
  | "cancelled"
  | "success"
  | "empty"
  | "http-error"
  | "network"
  | "timeout"
  | "pre-script-error"
  | "pre-script-timeout"
  | "failed";
