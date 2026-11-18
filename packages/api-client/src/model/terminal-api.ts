import type { TemporarySshRoute } from "@unfour/command-client";
import type { RequestDraft } from "./types";

export function isTerminalApiError(code: string | null | undefined): code is string {
  return Boolean(code?.startsWith("SSH_TERMINAL_"));
}

/** Transport completion cannot establish whether the remote HTTP action took effect. */
export function terminalApiOutcomeUnknown(code: string | null | undefined) {
  return ["SSH_TERMINAL_CANCELLED", "SSH_TERMINAL_TIMEOUT", "SSH_TERMINAL_TRANSPORT_CLOSED", "SSH_TERMINAL_OUTPUT_LIMIT", "SSH_TERMINAL_PROTOCOL_ERROR",
    "SSH_TERMINAL_INPUT_ACK_TIMEOUT", "SSH_TERMINAL_BEGIN_TIMEOUT", "SSH_TERMINAL_END_TIMEOUT",
    "SSH_TERMINAL_HTTP_TIMEOUT", "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE", "SSH_TERMINAL_HTTP_INVALID_RESPONSE", "SSH_TERMINAL_HTTP_UNSUPPORTED_RESPONSE", "SSH_TERMINAL_HTTP_TRANSFER_FAILED"].includes(code ?? "");
}

export function terminalApiSendError(draft: RequestDraft, session: TemporarySshRoute | undefined): string | null {
  if (!draft.temporaryHandle || draft.temporaryExecutionMode !== "terminalCurl") return null;
  if (!session) return "SSH_TERMINAL_HTTP_TARGET_REQUIRED";
  if (session.terminalRecoveryRequired) return "SSH_TERMINAL_RECOVERY_REQUIRED";
  if (session.terminalTaskBusy) return "SSH_TERMINAL_BUSY";
  if (!draft.terminalConfirmed) return "SSH_TERMINAL_HTTP_CONFIRMATION_REQUIRED";
  if (draft.bodyMode === "multipart") return "SSH_TERMINAL_HTTP_UNSUPPORTED_REQUEST";
  return null;
}
