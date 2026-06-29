// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SplitPane } from "./shell";
import { clampResizablePaneSize } from "./shell-utils";

vi.mock("react-resizable-panels", () => ({
  Group: ({ children, orientation }: { children: React.ReactNode; orientation: string }) => (
    <div data-panel-group orientation={orientation}>{children}</div>
  ),
  Panel: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-panel className={className}>{children}</div>
  ),
  Separator: ({ "aria-label": ariaLabel, className }: { "aria-label": string; className?: string }) => (
    <div aria-label={ariaLabel} className={className} data-panel-separator role="separator" />
  ),
}));

afterEach(cleanup);

describe("clampResizablePaneSize", () => {
  it("keeps resized shell pane widths within bounds", () => {
    expect(clampResizablePaneSize(180, 220, 420, 264)).toBe(220);
    expect(clampResizablePaneSize(320, 220, 420, 264)).toBe(320);
    expect(clampResizablePaneSize(460, 220, 420, 264)).toBe(420);
  });

  it("keeps the current width when the pointer measurement is invalid", () => {
    expect(clampResizablePaneSize(Number.NaN, 220, 420, 264)).toBe(264);
  });
});

describe("SplitPane", () => {
  it("renders resizable panes through the panel adapter", () => {
    render(
      <SplitPane resizable>
        <section>Primary</section>
        <section>Secondary</section>
      </SplitPane>,
    );

    expect(screen.getByText("Primary").closest("[data-panel]")).not.toBeNull();
    expect(screen.getByText("Secondary").closest("[data-panel]")).not.toBeNull();
    expect(screen.getByRole("separator", { name: "Resize horizontal split" })).toBeInTheDocument();
  });
});
