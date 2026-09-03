import type { ApiResponse } from "@unfour/command-client";
import { describe, expect, it } from "vitest";
import {
  classifyRequestError,
  deriveApiRequestState,
  formatError,
  formatResponseBody,
  looksLikeJson,
} from "./api-request-state";

describe("formatError", () => {
  it("uses the message of Error instances", () => {
    expect(formatError(new Error("boom"))).toBe("boom");
  });

  it("reads the message field of plain objects", () => {
    expect(formatError({ message: "nope" })).toBe("nope");
  });

  it("stringifies anything else", () => {
    expect(formatError("raw")).toBe("raw");
    expect(formatError(42)).toBe("42");
  });
});

describe("classifyRequestError", () => {
  it("prefers stable API error codes over localized messages", () => {
    expect(classifyRequestError({ code: "API_TIMEOUT", message: "localized" })).toBe(
      "timeout",
    );
    expect(classifyRequestError({ code: "API_CANCELLED", message: "localized" })).toBe(
      "cancelled",
    );
    expect(classifyRequestError({ code: "NETWORK_ERROR", message: "localized" })).toBe(
      "network",
    );
  });

  it("detects timeouts", () => {
    expect(classifyRequestError(new Error("Request timed out"))).toBe("timeout");
    expect(classifyRequestError("connection timeout")).toBe("timeout");
  });

  it("detects network-class failures", () => {
    expect(classifyRequestError(new Error("network unreachable"))).toBe(
      "network",
    );
    expect(classifyRequestError("DNS lookup failed")).toBe("network");
    expect(classifyRequestError("failed to fetch")).toBe("network");
  });

  it("falls back to failed for unknown errors", () => {
    expect(classifyRequestError(new Error("500 server error"))).toBe("failed");
  });
});

describe("deriveApiRequestState", () => {
  const base = {
    error: null as unknown,
    hasSelectedRequest: false,
    isSending: false,
    response: null as ApiResponse | null,
  };

  it("prioritizes the sending state", () => {
    expect(deriveApiRequestState({ ...base, isSending: true })).toBe("sending");
  });

  it("classifies errors when present", () => {
    expect(
      deriveApiRequestState({ ...base, error: new Error("timed out") }),
    ).toBe("timeout");
  });

  it("maps response status to success or failed", () => {
    expect(
      deriveApiRequestState({
        ...base,
        response: { status: 200 } as ApiResponse,
      }),
    ).toBe("success");
    expect(
      deriveApiRequestState({
        ...base,
        response: { status: 404 } as ApiResponse,
      }),
    ).toBe("failed");
  });

  it("distinguishes selected from new when idle", () => {
    expect(deriveApiRequestState({ ...base, hasSelectedRequest: true })).toBe(
      "selected",
    );
    expect(deriveApiRequestState(base)).toBe("new");
  });
});

describe("formatResponseBody", () => {
  it("returns an empty string for missing bodies", () => {
    expect(formatResponseBody()).toBe("");
    expect(formatResponseBody("")).toBe("");
  });

  it("pretty-prints valid JSON", () => {
    expect(formatResponseBody('{"a":1}')).toBe('{\n  "a": 1\n}');
  });

  it("returns the raw body when it is not JSON", () => {
    expect(formatResponseBody("plain text")).toBe("plain text");
  });
});

describe("looksLikeJson", () => {
  it("accepts objects and arrays", () => {
    expect(looksLikeJson('  {"a":1} ')).toBe(true);
    expect(looksLikeJson("[1,2,3]")).toBe(true);
  });

  it("rejects non-JSON and malformed input", () => {
    expect(looksLikeJson("")).toBe(false);
    expect(looksLikeJson("hello")).toBe(false);
    expect(looksLikeJson("{ broken")).toBe(false);
  });
});
