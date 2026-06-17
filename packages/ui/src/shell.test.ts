import { describe, expect, it } from "vitest";
import { clampResizablePaneSize } from "./shell";

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
