import type { ApiResponse } from "@unfour/command-client";
import { createTranslator } from "@unfour/ui";
import { describe, expect, it } from "vitest";
import {
  classifyRequestError,
  shouldAppendOutcomeWarning,
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
  it.each(["STARTUP", "READY", "ACCEPT", "INPUT_ACK", "BEGIN", "END", "RESTORE"])("localizes and classifies the %s terminal deadline", (phase) => {
    const code = `SSH_TERMINAL_${phase}_TIMEOUT`;
    for (const locale of ["en", "zh-CN"] as const) {
      const message = createTranslator(locale)(`network.errors.${code}`);
      expect(message).not.toContain(code);
      expect(classifyRequestError({ code, message })).toBe("timeout");
    }
    expect(shouldAppendOutcomeWarning(code)).toBe(["INPUT_ACK", "BEGIN", "END"].includes(phase));
  });
  it.each([
    ["SSH_TUNNEL_TARGET_CONNECT_FAILED", "network"],
    ["SSH_TUNNEL_TRANSPORT_CLOSED", "network"],
    ["SSH_TUNNEL_FORWARD_PROHIBITED", "failed"],
    ["SSH_TUNNEL_CHANNEL_UNSUPPORTED", "failed"],
    ["SSH_TUNNEL_RESOURCE_SHORTAGE", "failed"],
    ["SSH_TUNNEL_FORWARD_FAILED", "failed"],
  ])("classifies %s without interpreting translated server text", (code, expected) => {
    expect(classifyRequestError({ code, message: "connection timeout 转发失败" })).toBe(expected);
  });

  it("classifies tunnel errors consistently across display languages", () => {
    expect(classifyRequestError({ code: "SSH_TUNNEL_TIMEOUT", message: "经 SSH 的请求超时。" })).toBe("timeout");
    expect(classifyRequestError({ code: "SSH_TUNNEL_TARGET_UNREACHABLE", message: "无法连接目标" })).toBe("network");
    expect(classifyRequestError({ code: "SSH_TUNNEL_ROUTE_CHANGED", message: "The network route changed" })).toBe("failed");
  });

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
