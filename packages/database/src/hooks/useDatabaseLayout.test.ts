// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useDatabaseLayout } from "./useDatabaseLayout";

describe("useDatabaseLayout", () => {
  it("opens on the default SQL editor tab with results and columns selected", () => {
    const { result } = renderHook(() => useDatabaseLayout());
    expect(result.current.activeTabId).toBe("sql-editor");
    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.resultTab).toBe("results");
    expect(result.current.inspectorTab).toBe("columns");
  });

  it("updates the active, result, and inspector tabs", () => {
    const { result } = renderHook(() => useDatabaseLayout());

    act(() => result.current.setActiveTabId("table:users"));
    act(() => result.current.setResultTab("messages"));
    act(() => result.current.setInspectorTab("ddl"));

    expect(result.current.activeTabId).toBe("table:users");
    expect(result.current.resultTab).toBe("messages");
    expect(result.current.inspectorTab).toBe("ddl");
  });
});
