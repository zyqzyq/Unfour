// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useDatabaseLayout } from "./useDatabaseLayout";

describe("useDatabaseLayout", () => {
  it("opens on the query console with the data segment and results selected", () => {
    const { result } = renderHook(() => useDatabaseLayout());
    expect(result.current.activeTabId).toBe("query");
    expect(result.current.tableSegment).toBe("data");
    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.resultTab).toBe("results");
    expect(result.current.inspectorTab).toBe("ddl");
  });

  it("updates the active tab, table segment, result, and inspector tabs", () => {
    const { result } = renderHook(() => useDatabaseLayout());

    act(() => result.current.setActiveTabId("table"));
    act(() => result.current.setTableSegment("structure"));
    act(() => result.current.setResultTab("messages"));
    act(() => result.current.setInspectorTab("indexes"));

    expect(result.current.activeTabId).toBe("table");
    expect(result.current.tableSegment).toBe("structure");
    expect(result.current.resultTab).toBe("messages");
    expect(result.current.inspectorTab).toBe("indexes");
  });
});
