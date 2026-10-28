export type AccountFailureStage = "prepareSignIn" | "openBrowser" | "callbackListener"
  | "callbackValidation" | "codeExchange" | "credentialRead" | "credentialSave"
  | "accountRefresh" | "signOut";
export type AccountRecovery = "restartSignIn" | "refreshAccount" | "retrySignOut" | "restartApp";
export interface AccountFailure {
  code: string;
  stage: AccountFailureStage;
  category: "temporary" | "authorization" | "local" | "service" | "unknown";
  recovery: AccountRecovery;
}

// Only known codes cross into UI diagnostics. Never render arbitrary IPC/server
// messages, callback parameters, URLs, or platform errors as an error code.
const safeCodes = new Set([
  "invalid_configuration", "invalid_deep_link", "no_pending_authorization", "state_mismatch",
  "authorization_denied", "api_unavailable", "api_timeout", "invalid_api_response",
  "keychain_unavailable", "installation_id_unavailable", "corrupt_installation_id",
  "corrupt_stored_session", "state_unavailable", "signed_out", "browser_open_failed",
  "callback_listener_failed", "sign_out_sync_deactivation_failed", "sign_out_cleanup_failed",
  "invalid_request", "unauthorized", "forbidden", "not_found", "method_not_allowed",
  "request_error", "internal_error", "not_ready", "invalid_state", "pkce_mismatch",
  "authorization_code_expired", "authorization_code_used", "desktop_session_expired", "rate_limited",
]);
const stages = new Set<AccountFailureStage>([
  "prepareSignIn", "openBrowser", "callbackListener", "callbackValidation", "codeExchange",
  "credentialRead", "credentialSave", "accountRefresh", "signOut",
]);
const temporaryCodes = new Set(["api_unavailable", "api_timeout", "not_ready", "rate_limited"]);
const authorizationCodes = new Set([
  "no_pending_authorization", "state_mismatch", "authorization_denied", "signed_out", "unauthorized",
  "invalid_state", "pkce_mismatch", "authorization_code_expired", "authorization_code_used",
  "desktop_session_expired",
]);
const localCodes = new Set([
  "invalid_configuration", "keychain_unavailable", "installation_id_unavailable",
  "corrupt_installation_id", "corrupt_stored_session", "state_unavailable",
  "browser_open_failed", "callback_listener_failed", "sign_out_cleanup_failed",
  "sign_out_sync_deactivation_failed", "invalid_deep_link",
]);

function failureCategory(code: string): AccountFailure["category"] {
  return code === "unknown_error" ? "unknown" : temporaryCodes.has(code) ? "temporary"
    : authorizationCodes.has(code) ? "authorization"
      : localCodes.has(code) ? "local" : "service";
}

function failureRecovery(code: string, stage: AccountFailureStage, category: AccountFailure["category"]): AccountRecovery {
  return stage === "callbackListener" || code === "invalid_configuration"
    || code === "corrupt_installation_id" ? "restartApp"
    : stage === "signOut" ? "retrySignOut"
      : (stage === "accountRefresh" || stage === "credentialRead")
        && category !== "authorization" && code !== "corrupt_stored_session"
        ? "refreshAccount" : "restartSignIn";
}

export function accountFailure(error: unknown, fallbackStage: AccountFailureStage): AccountFailure {
  const detail = typeof error === "object" && error !== null ? error as Record<string, unknown> : {};
  const code = typeof detail.code === "string" && safeCodes.has(detail.code) ? detail.code : "unknown_error";
  const stage = stages.has(detail.stage as AccountFailureStage)
    ? detail.stage as AccountFailureStage : fallbackStage;
  const category = failureCategory(code);
  return { code, stage, category, recovery: failureRecovery(code, stage, category) };
}
