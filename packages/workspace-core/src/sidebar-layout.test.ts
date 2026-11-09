import { describe, expect, it } from "vitest";
import {
  DEFAULT_SIDEBAR_WIDTHS,
  MODULE_SIDEBAR_CONFIG,
  normalizeSidebarWidths,
} from "./sidebar-layout";

describe("sidebar layout normalization", () => {
  it("exposes the module-specific defaults and bounds", () => {
    expect(DEFAULT_SIDEBAR_WIDTHS).toEqual({ api: 320, ssh: 248, flow: 280, database: 280 });
    expect(MODULE_SIDEBAR_CONFIG).toEqual({
      api: { defaultWidth: 320, minWidth: 220, maxWidth: 560 },
      ssh: { defaultWidth: 248, minWidth: 220, maxWidth: 420 },
      flow: { defaultWidth: 280, minWidth: 220, maxWidth: 520 },
      database: { defaultWidth: 280, minWidth: 220, maxWidth: 520 },
    });
  });

  it("falls back to module defaults for invalid new values", () => {
    expect(
      normalizeSidebarWidths({ api: null, ssh: "250", database: Number.NaN }),
    ).toEqual({ api: 320, ssh: 248, flow: 280, database: 280 });
  });

  it("clamps a legacy global width independently for each module", () => {
    expect(normalizeSidebarWidths(undefined, 500)).toEqual({
      api: 500,
      ssh: 420,
      flow: 280, database: 500,
    });
  });

  it("rounds fractional widths to integer pixels for the Rust layout contract", () => {
    expect(normalizeSidebarWidths({ api: 320.4, ssh: 248.6, database: 280.5, flow: 300.25 }))
      .toEqual({ api: 320, ssh: 249, database: 281, flow: 300 });
    expect(normalizeSidebarWidths(undefined, 350.75))
      .toEqual({ api: 351, ssh: 351, database: 351, flow: 280 });
  });
});
