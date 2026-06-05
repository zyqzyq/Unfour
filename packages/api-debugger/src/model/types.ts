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

export type RequestParamsTab = "query" | "headers" | "body" | "auth";
export type ResponsePanelTab = "response" | "history";
export type ResponseTab = "body" | "headers" | "cookies" | "timing";

export type RequestDraft = {
  body: string;
  envVariables: KeyValue[];
  folderPath: string;
  headers: KeyValue[];
  method: string;
  name: string;
  query: KeyValue[];
  url: string;
};

export type ApiResponseSummary = {
  response: ApiResponse | null;
  responseCookies: KeyValue[];
};
