import type { TemporarySshRoute } from "@unfour/command-client";
import type { RequestDraft } from "./types";

export function isTerminalApiError(code: string | null | undefined): code is string {
  return Boolean(code?.startsWith("SSH_TERMINAL_"));
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
