// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SplitPane } from "./shell";
import { clampResizablePaneSize } from "./shell-utils";

vi.mock("react-resizable-panels", () => ({
  Group: ({ children, orientation }: { children: React.ReactNode; orientation: string }) => (
    <div data-panel-group orientation={orientation}>{children}</div>
  ),
  Panel: ({
    children,
    className,
    defaultSize,
    minSize,
  }: {
    children: React.ReactNode;
    className?: string;
    defaultSize?: number | string;
    minSize?: number | string;
  }) => (
    <div data-default-size={defaultSize} data-min-size={minSize} data-panel className={className}>
      {children}
    </div>
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

  it("passes split ratios to the adapter as percentages", () => {
    render(
      <SplitPane defaultRatio={62} minPaneSize={220} orientation="vertical" resizable>
        <section>SQL</section>
        <section>Results</section>
      </SplitPane>,
    );

    expect(screen.getByText("SQL").closest("[data-panel]")).toHaveAttribute("data-default-size", "62%");
    expect(screen.getByText("Results").closest("[data-panel]")).toHaveAttribute("data-default-size", "38%");
    expect(screen.getByText("SQL").closest("[data-panel]")).toHaveAttribute("data-min-size", "10%");
    expect(screen.getByRole("separator", { name: "Resize vertical split" })).toBeInTheDocument();
  });
});
