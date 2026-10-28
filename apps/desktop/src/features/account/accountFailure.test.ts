import { describe, expect, it } from "vitest";
import { accountFailure } from "./accountFailure";

describe("safe account diagnostics", () => {
  it.each([
    ["api_timeout", "codeExchange", "temporary", "restartSignIn"],
    ["api_unavailable", "accountRefresh", "temporary", "refreshAccount"],
    ["authorization_code_used", "codeExchange", "authorization", "restartSignIn"],
    ["desktop_session_expired", "accountRefresh", "authorization", "restartSignIn"],
    ["keychain_unavailable", "credentialSave", "local", "restartSignIn"],
    ["keychain_unavailable", "credentialRead", "local", "refreshAccount"],
    ["keychain_unavailable", "signOut", "local", "retrySignOut"],
    ["state_unavailable", "accountRefresh", "local", "refreshAccount"],
    ["keychain_unavailable", "accountRefresh", "local", "refreshAccount"],
    ["callback_listener_failed", "callbackListener", "local", "restartApp"],
    ["callback_read_failed", "callbackRead", "local", "restartSignIn"],
  ])("maps %s at %s to an explicit recovery", (code, stage, category, recovery) => {
    expect(accountFailure({ code, stage }, "prepareSignIn")).toEqual({ code, stage, category, recovery });
  });

  it("drops untrusted messages, codes, and stages instead of displaying secrets", () => {
    const secret = "unfour://auth/callback?authorizationCode=secret";
    const result = accountFailure({ code: secret, stage: secret, message: secret }, "callbackValidation");
    expect(result.code).toBe("unknown_error");
    expect(result.category).toBe("unknown");
    expect(result.stage).toBe("callbackValidation");
    expect(JSON.stringify(result)).not.toContain("secret");
    expect(accountFailure(new Error(secret), "prepareSignIn").code).toBe("unknown_error");
  });
});
