import type { ApiResponse } from "@unfour/command-client";
import type { ApiRequestState } from "./types";

export function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

export function classifyRequestError(error: unknown): "failed" | "network" | "timeout" {
  const message = formatError(error).toLowerCase();
  if (message.includes("timeout") || message.includes("timed out")) {
    return "timeout";
  }
  if (
    message.includes("network") ||
    message.includes("dns") ||
    message.includes("connection") ||
    message.includes("fetch")
  ) {
    return "network";
  }
  return "failed";
}

export function deriveApiRequestState({
  error,
  hasSelectedRequest,
  isSending,
  response,
}: {
  error: unknown;
  hasSelectedRequest: boolean;
  isSending: boolean;
  response: ApiResponse | null;
}): ApiRequestState {
  if (isSending) {
    return "sending";
  }
  if (error) {
    return classifyRequestError(error);
  }
  if (response) {
    return response.status < 400 ? "success" : "failed";
  }
  return hasSelectedRequest ? "selected" : "new";
}

export const apiRequestStateLabel: Record<ApiRequestState, string> = {
  failed: "failed",
  network: "network error",
  new: "new",
  selected: "selected",
  sending: "sending",
  success: "success",
  timeout: "timeout",
};

export const apiRequestStateTone: Record<ApiRequestState, "neutral" | "green" | "amber" | "red"> =
  {
    failed: "red",
    network: "red",
    new: "neutral",
    selected: "neutral",
    sending: "amber",
    success: "green",
    timeout: "amber",
  };

export function formatResponseBody(body?: string) {
  if (!body) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

export function looksLikeJson(value: string) {
  const trimmed = value.trim();
  if (!trimmed || (!trimmed.startsWith("{") && !trimmed.startsWith("["))) {
    return false;
  }
  try {
    JSON.parse(trimmed);
    return true;
  } catch {
    return false;
  }
}
