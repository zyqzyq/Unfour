// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useApiLayout } from "./useApiLayout";

describe("useApiLayout", () => {
  it("starts on the query, response, and body tabs", () => {
    const { result } = renderHook(() => useApiLayout());
    expect(result.current.requestTab).toBe("query");
    expect(result.current.resultTab).toBe("response");
    expect(result.current.responseTab).toBe("body");
  });

  it("updates each tab independently", () => {
    const { result } = renderHook(() => useApiLayout());

    act(() => result.current.setRequestTab("headers"));
    act(() => result.current.setResponseTab("headers"));

    expect(result.current.requestTab).toBe("headers");
    expect(result.current.responseTab).toBe("headers");
    // Unrelated tab state is left untouched.
    expect(result.current.resultTab).toBe("response");
  });
});
