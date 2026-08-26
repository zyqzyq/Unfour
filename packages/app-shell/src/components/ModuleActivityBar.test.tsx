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
