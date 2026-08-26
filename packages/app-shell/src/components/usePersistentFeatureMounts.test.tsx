// @vitest-environment jsdom
import type { WorkspaceTab } from "@unfour/command-client";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { usePersistentFeatureMounts } from "./usePersistentFeatureMounts";

const tabs: WorkspaceTab[] = [
  { id: "api-main", kind: "api", title: "API Client" },
  { id: "database-main", kind: "database", title: "Database" },
  { id: "ssh-main", kind: "ssh", title: "SSH Terminal" },
];

describe("usePersistentFeatureMounts", () => {
  it("keeps feature modules mounted after their first activation", () => {
    const setActiveTab = vi.fn();
    const { result, rerender } = renderHook(
      ({ activeTabId }: { activeTabId: string }) =>
        usePersistentFeatureMounts({ activeTabId, setActiveTab, tabs }),
      { initialProps: { activeTabId: "api-main" } },
    );

    expect(result.current).toMatchObject({
      shouldMountApi: true,
      shouldMountDatabase: false,
      shouldMountSsh: false,
    });

    act(() => result.current.setActiveTab("database-main"));
    expect(setActiveTab).toHaveBeenLastCalledWith("database-main");
    rerender({ activeTabId: "database-main" });
    expect(result.current).toMatchObject({
      shouldMountApi: true,
      shouldMountDatabase: true,
      shouldMountSsh: false,
    });

    act(() => result.current.setActiveTab("ssh-main"));
    rerender({ activeTabId: "ssh-main" });
    expect(result.current).toMatchObject({
      shouldMountApi: true,
      shouldMountDatabase: true,
      shouldMountSsh: true,
    });
  });

  it("persists SSH when it is the initial feature", () => {
    const setActiveTab = vi.fn();
    const { result, rerender } = renderHook(
      ({ activeTabId }: { activeTabId: string }) =>
        usePersistentFeatureMounts({ activeTabId, setActiveTab, tabs }),
      { initialProps: { activeTabId: "ssh-main" } },
    );

    expect(result.current).toMatchObject({
      shouldMountApi: false,
      shouldMountDatabase: false,
      shouldMountSsh: true,
    });

    rerender({ activeTabId: "database-main" });
    act(() => result.current.setActiveTab("api-main"));
    rerender({ activeTabId: "ssh-main" });
    expect(result.current).toEqual({
      setActiveTab: expect.any(Function),
      shouldMountApi: true,
      shouldMountDatabase: true,
      shouldMountSsh: true,
    });
  });
});
