import { describe, expect, it } from "vitest";
import type { RequestDraft } from "../model/types";
import {
  createNewRequestTab,
  emptyApiTabsState,
  type ApiRequestTab,
} from "../model/request-tabs";
import { tabToInput } from "./useApiRequestTabs";

const WORKSPACE = "ws-1";

function tabWithDraft(overrides: Partial<RequestDraft>): ApiRequestTab {
  const state = createNewRequestTab(emptyApiTabsState(WORKSPACE), "new:1");
  const tab = state.tabs[0];
  return { ...tab, draft: { ...tab.draft, ...overrides } };
}

function header(input: ReturnType<typeof tabToInput>, key: string) {
  return input.headers.find(
    (item) => item.key.toLowerCase() === key.toLowerCase(),
  );
}

describe("tabToInput", () => {
  it("strips the query string from the url and omits the body for GET", () => {
    const input = tabToInput(
      tabWithDraft({ method: "GET", url: "https://api.test/items?page=2", body: "x" }),
      WORKSPACE,
    );

    expect(input.url).toBe("https://api.test/items");
    expect(input.body).toBeUndefined();
    expect(input.workspaceId).toBe(WORKSPACE);
  });

  it("adds a JSON content-type and keeps the body for raw JSON POSTs", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "POST",
        url: "https://api.test/items",
        bodyMode: "raw",
        rawBodyType: "json",
        body: '{"a":1}',
      }),
      WORKSPACE,
    );

    expect(header(input, "Content-Type")?.value).toBe("application/json");
    expect(input.body).toBe('{"a":1}');
  });

  it("generates a Bearer Authorization header from bearer auth", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "POST",
        url: "https://api.test",
        auth: { type: "bearer", token: "abc123" },
      }),
      WORKSPACE,
    );

    expect(header(input, "Authorization")?.value).toBe("Bearer abc123");
  });


  it("resolves environment variables inside generated bearer auth", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "POST",
        url: "https://api.test",
        auth: { type: "bearer", token: "{{access_token}}" },
      }),
      WORKSPACE,
      {
        envVariables: [{ enabled: true, key: "access_token", value: "abc123" }],
      },
    );

    expect(header(input, "Authorization")?.value).toBe("Bearer abc123");
  });

  it("generates a Basic Authorization header from basic auth", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "POST",
        url: "https://api.test",
        auth: { type: "basic", username: "user", password: "pass" },
      }),
      WORKSPACE,
    );

    expect(header(input, "Authorization")?.value).toBe(
      `Basic ${btoa("user:pass")}`,
    );
  });

  it("places an api-key into the query when configured for the query", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "GET",
        url: "https://api.test",
        auth: { type: "api-key", addTo: "query", key: "api_key", value: "secret" },
      }),
      WORKSPACE,
    );

    expect(input.query.find((item) => item.key === "api_key")?.value).toBe(
      "secret",
    );
  });


  it("resolves environment variables inside generated api-key auth", () => {
    const queryInput = tabToInput(
      tabWithDraft({
        method: "GET",
        url: "https://api.test",
        auth: { type: "api-key", addTo: "query", key: "api_key", value: "{{api_key}}" },
      }),
      WORKSPACE,
      {
        envVariables: [{ enabled: true, key: "api_key", value: "secret" }],
      },
    );
    const headerInput = tabToInput(
      tabWithDraft({
        method: "GET",
        url: "https://api.test",
        auth: { type: "api-key", addTo: "header", key: "X-API-Key", value: "{{api_key}}" },
      }),
      WORKSPACE,
      {
        envVariables: [{ enabled: true, key: "api_key", value: "secret" }],
      },
    );

    expect(queryInput.query.find((item) => item.key === "api_key")?.value).toBe("secret");
    expect(header(headerInput, "X-API-Key")?.value).toBe("secret");
  });

  it("preserves an explicit Authorization header over generated auth", () => {
    const input = tabToInput(
      tabWithDraft({
        method: "POST",
        url: "https://api.test",
        headers: [{ enabled: true, key: "Authorization", value: "Bearer manual" }],
        auth: { type: "bearer", token: "generated" },
      }),
      WORKSPACE,
    );

    const authHeaders = input.headers.filter(
      (item) => item.key.toLowerCase() === "authorization",
    );
    expect(authHeaders).toHaveLength(1);
    expect(authHeaders[0].value).toBe("Bearer manual");
  });
});
