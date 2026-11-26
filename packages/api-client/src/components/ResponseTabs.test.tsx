// @vitest-environment jsdom
import type { ReactNode } from "react";
import type {
  ApiExecutedRequestSnapshot,
  ApiResponse,
  RequestExecutionResult,
} from "@unfour/command-client";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createTranslator, I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createNewRequestTab, emptyApiTabsState, type ApiRequestTab } from "../model/request-tabs";
import { ResponseTabs } from "./ResponseTabs";

vi.mock("@monaco-editor/react", () => ({
  default: ({ value }: { value: string }) => (
    <textarea aria-label="mock editor" readOnly value={value} />
  ),
}));

afterEach(cleanup);

function withI18n(children: ReactNode) {
  return <I18nProvider initialLocale="en">{children}</I18nProvider>;
}

function baseTab(overrides: Partial<ApiRequestTab> = {}): ApiRequestTab {
  const state = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
  return { ...state.tabs[0], ...overrides };
}

describe("ResponseTabs", () => {
  it("keeps a failed cancellation visible without claiming the pending request has ended", () => {
    render(withI18n(<ResponseTabs onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} onRetry={vi.fn()}
      tab={baseTab({ sending: true, cancelling: false, cancelError: "Cancellation was not confirmed. Try Stop again." })} />));
    expect(screen.getByRole("alert")).toHaveTextContent("Cancellation was not confirmed");
    expect(screen.queryByText("Request cancelled")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });
  it("renders cancellation as its own state instead of a generic failure", () => {
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({
            sendError: "API request cancelled",
            sendErrorCode: "API_CANCELLED",
          })}
        />,
      ),
    );

    expect(screen.getByText("Request cancelled")).toBeInTheDocument();
    expect(screen.queryByText("Request failed")).toBeNull();
  });

  it("wraps and pretty-prints HTTP error details", () => {
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({ response: response({ status: 401, body: '{"error":"invalid_token","message":"Token expired"}' }) })}
        />,
      ),
    );

    const details = screen.getByText(/invalid_token/).closest("pre");

    expect(details?.textContent).toContain(`
  "error"`);
    expect(details).toHaveClass("whitespace-pre-wrap");
    expect(details).toHaveClass("break-words");
  });

  describe.each(["en", "zh-CN"] as const)("HTTP errors in %s", (locale) => {
    it.each([
      [400, '{"error":"JSON EOF while parsing a value"}'],
      [404, ""],
      [429, "Rate limit exceeded"],
      [500, "Internal server error"],
    ])("keeps HTTP %i details without suggesting Auth settings", (status, body) => {
      const t = createTranslator(locale);
      const openAuth = vi.fn();
      const retry = vi.fn();
      const { container } = render(
        <I18nProvider initialLocale={locale}>
          <ResponseTabs onOpenAuthSettings={openAuth} onResponseTabChange={vi.fn()} onRetry={retry}
            tab={baseTab({ response: response({ status, body }) })} />
        </I18nProvider>,
      );

      expect(screen.getByRole("heading", { name: t("api.response.httpErrorTitle", { status }) })).toBeInTheDocument();
      expect(screen.getByText(t("api.response.httpErrorDescription"))).toBeInTheDocument();
      expect(screen.queryByText(t("api.response.rejectedDescription"))).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: t("api.response.openAuthSettings") })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: t("api.response.retry") })).not.toBeInTheDocument();
      if (status === 400) {
        expect(JSON.parse(container.querySelector("pre")!.textContent!)).toEqual(JSON.parse(body));
      } else if (body) {
        expect(container.querySelector("pre")?.textContent).toBe(body);
      } else {
        expect(container.querySelector("pre")).toBeNull();
      }
      expect(openAuth).not.toHaveBeenCalled();
      expect(retry).not.toHaveBeenCalled();
    });

    it.each([401, 403])("offers Auth settings only for explicit HTTP %i", (status) => {
      const t = createTranslator(locale);
      const openAuth = vi.fn();
      render(
        <I18nProvider initialLocale={locale}>
          <ResponseTabs onOpenAuthSettings={openAuth} onResponseTabChange={vi.fn()} onRetry={vi.fn()}
            tab={baseTab({ response: response({ status, body: "Access denied" }) })} />
        </I18nProvider>,
      );

      expect(screen.getByText("Access denied")).toBeInTheDocument();
      expect(screen.getByText(t("api.response.rejectedDescription"))).toBeInTheDocument();
      expect(openAuth).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: t("api.response.openAuthSettings") }));
      expect(openAuth).toHaveBeenCalledOnce();
    });
  });

  it("shows terminal phases alongside the remote curl measurements in Timing", () => {
    const terminalTiming = { prepareMs: 11, inputMs: 502, inputWireBytes: 1041, inputBurst: true, inputAckCount: 1, inputAckWaitMs: 499,
      executionAndOutputMs: 244, curlTotalMs: 201, curlStartTransferMs: 198, restoreMs: 13 };
    render(withI18n(<ResponseTabs onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} onRetry={vi.fn()}
      tab={baseTab({ response: response({ terminalTiming }), responseTab: "timing" })} />));
    expect(screen.getByText("Input ACK count").nextElementSibling).toHaveTextContent("1");
    expect(screen.getByText("Request frame size").nextElementSibling).toHaveTextContent("1041 B");
    expect(screen.getByText("Upload mode").nextElementSibling).toHaveTextContent("Verified frame");
    expect(screen.getByText("Time waiting for input ACKs").nextElementSibling).toHaveTextContent("499 ms");
    expect(screen.getByText("curl HTTP transfer").nextElementSibling).toHaveTextContent("201 ms");
    expect(screen.getByText("curl time to first byte").nextElementSibling).toHaveTextContent("198 ms");
  });

  it("shows ordinary values and credential masks exactly as supplied by the request snapshot", () => {
    const snapshot: ApiExecutedRequestSnapshot = {
      method: "POST",
      url: "https://api.test/users?author=alice&access_token=%3Credacted%3E",
      query: [
        { enabled: true, key: "author", value: "alice" },
        { enabled: true, key: "access_token", value: "<redacted>" },
      ],
      headers: [
        { enabled: true, key: "X-Author-Id", value: "42" },
        { enabled: true, key: "Authorization", value: "<redacted>" },
      ],
      body: '{"name":"Ada"}',
      bodyKind: "json",
    };
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({
            lastRequest: snapshot,
            response: response({ body: "{}" }),
            responseTab: "request",
          })}
        />,
      ),
    );

    expect(screen.getByRole("button", { name: "Request" })).toBeInTheDocument();
    expect(screen.getByText("POST")).toBeInTheDocument();
    expect(screen.getByText(snapshot.url)).toBeInTheDocument();
    expect(screen.getByText("author").nextElementSibling?.textContent).toBe("alice");
    expect(screen.getByText("access_token").nextElementSibling?.textContent).toBe("<redacted>");
    expect(screen.getByText("X-Author-Id").nextElementSibling?.textContent).toBe("42");
    expect(screen.getByText("Authorization").nextElementSibling?.textContent).toBe("<redacted>");
    expect(screen.getByText('{"name":"Ada"}')).toBeInTheDocument();
  });

  it("shows script test pass/fail details and execution times", () => {
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({
            execution: execution(),
            response: response({ body: "{}" }),
            responseTab: "tests",
          })}
        />,
      ),
    );

    expect(screen.getByText("1 passed")).toBeInTheDocument();
    expect(screen.getByText("1 failed")).toBeInTheDocument();
    expect(screen.getByText("status is 200")).toBeInTheDocument();
    expect(screen.getByText("payload has id")).toBeInTheDocument();
    expect(screen.getByText("expected property id")).toBeInTheDocument();
    expect(screen.getByText("Pre 2ms · Post 4ms")).toBeInTheDocument();
  });

  it("shows phase-tagged console output and a typed script error", () => {
    const scripted = execution();
    scripted.postResponse.status = "failed";
    scripted.postResponse.error = {
      kind: "runtime",
      code: "SCRIPT_RUNTIME_ERROR",
      message: "post failed",
    };
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({
            execution: scripted,
            response: response({ body: "{}" }),
            responseTab: "console",
          })}
        />,
      ),
    );

    expect(screen.getByText("pre message")).toBeInTheDocument();
    expect(screen.getByText("post warning")).toBeInTheDocument();
    expect(screen.getByText("post failed")).toBeInTheDocument();
    expect(screen.getAllByText("post").length).toBeGreaterThan(0);
  });

  it("shows a prominent pre-request error before any response exists", () => {
    const scripted = execution();
    scripted.response = null;
    scripted.preRequest.status = "failed";
    scripted.preRequest.error = {
      kind: "runtime",
      code: "SCRIPT_RUNTIME_ERROR",
      message: "request setup failed",
    };
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({ execution: scripted, response: null, responseTab: "body" })}
        />,
      ),
    );

    expect(screen.getAllByText("Pre-request script failed")).toHaveLength(2);
    expect(screen.getByText("request setup failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});

function execution(): RequestExecutionResult {
  return {
    response: response({ body: "{}" }),
    httpError: null,
    httpErrorCode: null,
    preRequest: {
      status: "success",
      durationMs: 2,
      console: [{ level: "log", message: "pre message", sequence: 0 }],
      tests: [],
      error: null,
    },
    postResponse: {
      status: "success",
      durationMs: 4,
      console: [{ level: "warn", message: "post warning", sequence: 0 }],
      tests: [
        { name: "status is 200", passed: true, errorMessage: null, durationMs: 1 },
        {
          name: "payload has id",
          passed: false,
          errorMessage: "expected property id",
          durationMs: 1,
        },
      ],
      error: null,
    },
  };
}

function response(overrides: Partial<ApiResponse> = {}): ApiResponse {
  return {
    historyId: "history-1",
    status: 200,
    statusText: "OK",
    headers: [],
    body: "",
    durationMs: 12,
    ...overrides,
  };
}

it.each([
  ["network unavailable", "api.response.networkTitle"],
  ["request timeout", "api.response.timeoutTitle"],
  ["invalid request", "api.response.failedTitle"],
])("keeps the failure message and explicit retry action for %s", (sendError, title) => {
  const base = baseTab();
  const retry = vi.fn();
  const tab = { ...base, sendError, draft: { ...base.draft, url: "https://example.test" } };
  render(withI18n(<ResponseTabs tab={tab} onRetry={retry} onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} />));
  expect(screen.getByText(createTranslator("en")(title))).toBeInTheDocument();
  expect(screen.getByText(sendError)).toBeInTheDocument();
  expect(retry).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(retry).toHaveBeenCalledTimes(1);
});

it.each(["request", "tests", "console"] as const)("keeps failure details and retry visible on %s without changing the view", (responseTab) => {
  const base = baseTab();
  const retry = vi.fn();
  const selectView = vi.fn();
  const tab = { ...base, responseTab, execution: execution(), sendError: "DNS lookup failed", sendErrorCode: "NETWORK_ERROR", draft: { ...base.draft, url: "https://example.test" } };
  const { rerender } = render(withI18n(<ResponseTabs tab={tab} onRetry={retry} onOpenAuthSettings={vi.fn()} onResponseTabChange={selectView} />));
  expect(screen.getByRole("alert")).toHaveTextContent("DNS lookup failed");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(retry).toHaveBeenCalledOnce();
  expect(selectView).not.toHaveBeenCalled();
  rerender(withI18n(<ResponseTabs tab={{ ...tab, sending: true }} onRetry={retry} onOpenAuthSettings={vi.fn()} onResponseTabChange={selectView} />));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
});

it("exposes pre-request script failure while keeping the console available", () => {
  const scripted = execution();
  scripted.preRequest.status = "failed";
  scripted.preRequest.error = { kind: "runtime", code: "SCRIPT_RUNTIME_ERROR", message: "Invalid setup" };
  render(withI18n(<ResponseTabs tab={baseTab({ execution: scripted, responseTab: "console" })} onRetry={vi.fn()} onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} />));
  expect(screen.getByRole("alert")).toHaveTextContent("Invalid setup");
  expect(screen.getByText("pre message")).toBeInTheDocument();
});
