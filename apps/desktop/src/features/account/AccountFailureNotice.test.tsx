// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { accountFailure, type AccountFailure } from "./accountFailure";
import { accountI18nResources } from "./locales";

const mocks = vi.hoisted(() => ({ failure: null as AccountFailure | null, retry: vi.fn() }));
vi.mock("./useAccount", () => ({
  useAccount: () => ({ failure: mocks.failure, state: { kind: "error" }, retry: mocks.retry, refreshing: false }),
}));
import { AccountFailureNotice, AccountRecoveryButton } from "./AccountFailureNotice";

afterEach(cleanup);

function show(code: string, stage: string, locale: "en" | "zh-CN") {
  mocks.failure = accountFailure({ code, stage, message: "secret-session-token" }, "codeExchange");
  return render(<I18nProvider initialLocale={locale} resources={accountI18nResources} storageKey={`failure-${locale}`}>
    <AccountFailureNotice /><AccountRecoveryButton />
  </I18nProvider>);
}

describe("account error presentation", () => {
  it("shows safe exchange diagnostics and a fresh sign-in action in English", () => {
    show("api_timeout", "codeExchange", "en");
    expect(screen.getByRole("alert")).toHaveTextContent("api_timeout");
    expect(screen.getByRole("alert")).toHaveTextContent("Exchange authorization code");
    expect(screen.getByRole("alert")).toHaveTextContent("may already be consumed");
    expect(screen.getByRole("alert")).not.toHaveTextContent("secret-session-token");
    fireEvent.click(screen.getByRole("button", { name: "Start new sign-in" }));
    expect(mocks.retry).toHaveBeenCalledTimes(1);
  });

  it("shows credential-save recovery in Chinese without replaying the exchange", () => {
    show("keychain_unavailable", "credentialSave", "zh-CN");
    expect(screen.getByRole("alert")).toHaveTextContent("保存凭据");
    expect(screen.getByRole("alert")).toHaveTextContent("系统凭据服务");
    expect(screen.getByRole("button", { name: "重新发起登录" })).toBeInTheDocument();
  });

  it("labels a read retry as account refresh and explains its limit", () => {
    show("api_unavailable", "accountRefresh", "en");
    expect(screen.getByRole("button", { name: "Refresh account" })).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("does not restart sign-in");
  });

  it("requires app recovery when the callback listener cannot be registered", () => {
    show("callback_listener_failed", "callbackListener", "en");
    expect(screen.getByRole("alert")).toHaveTextContent("Restart Unfour");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it.each([
    ["en", "Read startup callback", "Start new sign-in"],
    ["zh-CN", "读取启动回调", "重新发起登录"],
  ] as const)("offers fresh sign-in after startup callback lookup fails in %s", (locale, stage, action) => {
    show("callback_read_failed", "callbackRead", locale);
    expect(screen.getByRole("alert")).toHaveTextContent("callback_read_failed");
    expect(screen.getByRole("alert")).toHaveTextContent(stage);
    expect(screen.getByRole("button", { name: action })).toBeInTheDocument();
  });
});
