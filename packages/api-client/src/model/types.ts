import type {
  ApiResponse,
  ApiSavedRequest,
  KeyValue,
} from "@unfour/command-client";

export type ApiResourceGroup = {
  folder: string;
  items: ApiSavedRequest[];
};

export type ApiRequestState =
  | "new"
  | "selected"
  | "sending"
  | "success"
  | "failed"
  | "network"
  | "timeout";

export type RequestParamsTab = "query" | "auth" | "headers" | "body";
export type ResponsePanelTab = "response" | "history";
export type ResponseTab = "body" | "headers" | "cookies" | "timing";
export type ApiSplitDirection = "vertical" | "horizontal";
export type ApiTabSource = "new" | "saved" | "history";
export type ApiTabSaveState = "unsaved" | "saved" | "dirty" | "saving";
export type ApiOpenIntent =
  | { kind: "new"; nonce: number }
  | { action?: "open" | "send"; kind: "saved"; nonce: number; requestId: string }
  | { action?: "open" | "save"; historyId: string; kind: "history"; nonce: number };

export type RequestBodyMode = "none" | "raw" | "form";
export type RequestRawBodyType = "json" | "text";
export type ApiAuthPlacement = "header" | "query";
export type ApiAuthConfig =
  | { type: "none" }
  | { token: string; type: "bearer" }
  | { password: string; type: "basic"; username: string }
  | { addTo: ApiAuthPlacement; key: string; type: "api-key"; value: string };

export type RequestDraft = {
  auth: ApiAuthConfig;
  body: string;
  bodyMode: RequestBodyMode;
  envVariables: KeyValue[];
  folderPath: string;
  formBody: KeyValue[];
  headers: KeyValue[];
  method: string;
  name: string;
  query: KeyValue[];
  rawBodyType: RequestRawBodyType;
  url: string;
};

export type ApiResponseSummary = {
  response: ApiResponse | null;
  responseCookies: KeyValue[];
};
