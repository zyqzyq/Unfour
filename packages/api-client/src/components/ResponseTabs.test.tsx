// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiRequestInput, ApiResponse } from "@unfour/command-client";
import { cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
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
});

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
    folderPath: null,
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
