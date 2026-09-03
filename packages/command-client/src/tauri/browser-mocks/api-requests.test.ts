// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApiClientPreferences, ApiRequestInput, RequestExecutionResult } from "../../types";
import { handleApiRequestMock } from "./api-requests";
import { mockStore, mockWorkspace } from "./state";

describe("API request browser mock execution control", () => {
  beforeEach(() => {
    localStorage.clear();
    mockStore.history = [];
    mockStore.historyDetails = [];
    mockStore.savedRequests = [];
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("persists a zero-default local preference", async () => {
    const initial = await handleApiRequestMock<ApiClientPreferences>(
      "api_client_preferences_get",
    );
    expect(initial).toEqual({ requestTimeoutMs: 0 });

    await handleApiRequestMock("api_client_preferences_update", {
      preferences: { requestTimeoutMs: 120_000 },
    });
    const reloaded = await handleApiRequestMock<ApiClientPreferences>(
      "api_client_preferences_get",
    );
    expect(reloaded).toEqual({ requestTimeoutMs: 120_000 });
  });

  it("cancels only the selected execution and writes history only for success", async () => {
    let resolveSecond!: () => void;
    vi.stubGlobal(
      "fetch",
      vi.fn((url: URL | RequestInfo, init?: RequestInit) => {
        if (String(url).includes("/first")) {
          return rejectedOnAbort(init?.signal);
        }
        return new Promise<Response>((resolve) => {
          resolveSecond = () => resolve(new Response("ok", { status: 200 }));
        });
      }),
    );

    const first = send("first", input("https://example.test/first", 0));
    const second = send("second", input("https://example.test/second", 0));
    expect(
      await handleApiRequestMock<boolean>("api_cancel_request", {
        executionId: "unknown",
      }),
    ).toBe(false);
    expect(
      await handleApiRequestMock<boolean>("api_cancel_request", {
        executionId: "first",
      }),
    ).toBe(true);
    resolveSecond();

    expect((await first).httpErrorCode).toBe("API_CANCELLED");
    expect((await second).httpErrorCode).toBeNull();
    expect(mockStore.history).toHaveLength(1);
  });

  it("lets custom zero override a positive global timeout", async () => {
    await handleApiRequestMock("api_client_preferences_update", {
      preferences: { requestTimeoutMs: 5 },
    });
    vi.stubGlobal("fetch", vi.fn((_url: URL | RequestInfo, init?: RequestInit) =>
      rejectedOnAbort(init?.signal)));

    const execution = send("unlimited", input("https://example.test/slow", 0));
    await new Promise((resolve) => globalThis.setTimeout(resolve, 15));
    await handleApiRequestMock("api_cancel_request", { executionId: "unlimited" });

    expect((await execution).httpErrorCode).toBe("API_CANCELLED");
  });

  it("classifies an inherited positive timeout without writing history", async () => {
    await handleApiRequestMock("api_client_preferences_update", {
      preferences: { requestTimeoutMs: 5 },
    });
    vi.stubGlobal("fetch", vi.fn((_url: URL | RequestInfo, init?: RequestInit) =>
      rejectedOnAbort(init?.signal)));

    const result = await send("timed", input("https://example.test/slow", null));

    expect(result.httpErrorCode).toBe("API_TIMEOUT");
    expect(mockStore.history).toHaveLength(0);
  });
});

async function send(
  executionId: string,
  request: ApiRequestInput,
): Promise<RequestExecutionResult> {
  return await handleApiRequestMock<RequestExecutionResult>("api_send_request_v2", {
    executionId,
    input: request,
  }) as RequestExecutionResult;
}

function input(url: string, timeoutMs: number | null): ApiRequestInput {
  return {
    workspaceId: mockWorkspace.id,
    method: "GET",
    url,
    headers: [],
    query: [],
    bodyKind: "none",
    timeoutMs,
  };
}

function rejectedOnAbort(signal?: AbortSignal | null): Promise<Response> {
  return new Promise((_resolve, reject) => {
    signal?.addEventListener(
      "abort",
      () => reject(new DOMException("Aborted", "AbortError")),
      { once: true },
    );
  });
}
