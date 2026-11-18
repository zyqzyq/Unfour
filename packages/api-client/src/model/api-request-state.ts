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

export function errorCode(error: unknown): string | null {
  if (typeof error !== "object" || !error || !("code" in error)) {
    return null;
  }
  return String((error as { code: unknown }).code);
}

export function classifyRequestError(
  error: unknown,
): "cancelled" | "failed" | "network" | "timeout" {
  const code = errorCode(error);
  if (code === "API_CANCELLED" || code === "SSH_TERMINAL_CANCELLED") return "cancelled";
  if (code === "API_TIMEOUT" || code === "SSH_TUNNEL_TIMEOUT" || code === "SSH_TERMINAL_TIMEOUT" || code === "SSH_TERMINAL_HTTP_TIMEOUT") return "timeout";
  if (["SSH_TERMINAL_STARTUP_TIMEOUT", "SSH_TERMINAL_READY_TIMEOUT", "SSH_TERMINAL_ACCEPT_TIMEOUT", "SSH_TERMINAL_INPUT_ACK_TIMEOUT", "SSH_TERMINAL_BEGIN_TIMEOUT", "SSH_TERMINAL_END_TIMEOUT", "SSH_TERMINAL_RESTORE_TIMEOUT"].includes(code ?? "")) return "timeout";
  if (["NETWORK_ERROR", "SSH_TUNNEL_CONNECT_FAILED", "SSH_TUNNEL_TARGET_UNREACHABLE", "SSH_TUNNEL_TARGET_CONNECT_FAILED", "SSH_TUNNEL_TRANSPORT_CLOSED"].includes(code ?? "")) return "network";
  if (["SSH_TERMINAL_HTTP_DNS_FAILED", "SSH_TERMINAL_HTTP_CONNECT_FAILED", "SSH_TERMINAL_TRANSPORT_CLOSED"].includes(code ?? "")) return "network";
  if (code?.startsWith("SSH_TUNNEL_") || code?.startsWith("SSH_TERMINAL_")) return "failed";
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
  cancelled: "cancelled",
  cancelling: "cancelling",
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
    cancelled: "neutral",
    cancelling: "amber",
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
