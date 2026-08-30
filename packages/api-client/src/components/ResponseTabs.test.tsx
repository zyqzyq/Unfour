// @vitest-environment jsdom
import type { ReactNode } from "react";
import type {
  ApiRequestInput,
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

  it("shows the latest request snapshot next to the response", () => {
    render(
      withI18n(
        <ResponseTabs
          onOpenAuthSettings={vi.fn()}
          onResponseTabChange={vi.fn()}
          onRetry={vi.fn()}
          tab={baseTab({
            lastRequest: requestInput(),
            response: response({ body: "{}" }),
            responseTab: "request",
          })}
        />,
      ),
    );

    expect(screen.getByRole("button", { name: "Request" })).toBeInTheDocument();
    expect(screen.getByText("POST")).toBeInTheDocument();
    expect(screen.getByText("https://api.test/users")).toBeInTheDocument();
    expect(screen.getByText("Authorization")).toBeInTheDocument();
    expect(screen.getByText("<redacted>")).toBeInTheDocument();
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

function requestInput(): ApiRequestInput {
  return {
    workspaceId: "ws-1",
    name: "Create user",
    parentFolderId: null,
    collectionId: null,
    method: "POST",
    url: "https://api.test/users",
    headers: [
      { enabled: true, key: "Authorization", value: "Bearer secret" },
      { enabled: true, key: "Content-Type", value: "application/json" },
    ],
    query: [{ enabled: true, key: "page", value: "1" }],
    body: '{"name":"Ada"}',
    bodyKind: "json",
    timeoutMs: 60_000,
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
