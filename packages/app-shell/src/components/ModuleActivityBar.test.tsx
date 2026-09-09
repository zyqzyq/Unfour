// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModuleActivityBar } from "./ModuleActivityBar";

vi.mock("@unfour/ui", () => ({
  ActivityBar: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  cn: (...values: Array<string | false | undefined>) => values.filter(Boolean).join(" "),
  useI18n: () => ({ t: (_key: string, fallback?: string) => fallback ?? _key }),
}));

afterEach(cleanup);

describe("ModuleActivityBar", () => {
  it("navigates once from environment management, without a second sidebar action", () => {
    const onSelect = vi.fn();
    const onToggleSidebar = vi.fn();
    render(<ModuleActivityBar activeKind={null} onOpenCommandPalette={vi.fn()}
      onSelect={onSelect} onToggleSidebar={onToggleSidebar} sidebarCollapsed />);
    const api = screen.getByRole("button", { name: "API Client" });
    expect(api).not.toHaveAttribute("aria-current");
    fireEvent.click(api);
    expect(onSelect).toHaveBeenCalledExactlyOnceWith("api-main");
    expect(onToggleSidebar).not.toHaveBeenCalled();
  });

  it("preserves a collapsed sidebar when switching modules", () => {
    const onSelect = vi.fn();
    const onToggleSidebar = vi.fn();
    render(<ModuleActivityBar activeKind="api" onOpenCommandPalette={vi.fn()}
      onSelect={onSelect} onToggleSidebar={onToggleSidebar} sidebarCollapsed />);
    fireEvent.click(screen.getByRole("button", { name: "SSH Terminal" }));
    expect(onSelect).toHaveBeenCalledExactlyOnceWith("ssh-main");
    expect(onToggleSidebar).not.toHaveBeenCalled();
  });

  it("preloads a module when navigation intent is shown", () => {
    const onPreload = vi.fn();
    render(
      <ModuleActivityBar
        activeKind="api"
        onOpenCommandPalette={vi.fn()}
        onPreload={onPreload}
        onSelect={vi.fn()}
        onToggleSidebar={vi.fn()}
        sidebarCollapsed={false}
      />,
    );

    fireEvent.pointerEnter(screen.getByRole("button", { name: "SSH Terminal" }));
    expect(onPreload).toHaveBeenCalledWith("ssh");
  });
});
