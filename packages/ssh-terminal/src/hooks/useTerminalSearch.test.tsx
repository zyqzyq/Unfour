// @vitest-environment jsdom
import { act, fireEvent, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useTerminalSearch } from "./useTerminalSearch";
import { useTerminalStore } from "../model/terminal-state";

afterEach(() => {
  act(() => {
    useTerminalStore.getState().setSearchOpen(false);
    useTerminalStore.getState().setSearchQuery("");
  });
});

describe("useTerminalSearch", () => {
  it("opens the search bar on Ctrl+F", () => {
    const { result } = renderHook(() => useTerminalSearch());
    expect(result.current.open).toBe(false);

    act(() => {
      fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    });

    expect(result.current.open).toBe(true);
  });

  it("closes the search bar on Escape when it is open", () => {
    const { result } = renderHook(() => useTerminalSearch());
    act(() => result.current.setOpen(true));

    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });

    expect(result.current.open).toBe(false);
  });

  it("ignores Escape while the search bar is closed", () => {
    const { result } = renderHook(() => useTerminalSearch());

    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });

    expect(result.current.open).toBe(false);
  });

  it("exposes the shared query state through the store", () => {
    const { result } = renderHook(() => useTerminalSearch());

    act(() => result.current.setQuery("error"));
    expect(result.current.query).toBe("error");
  });
});
