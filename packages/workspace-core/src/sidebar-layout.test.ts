import { describe, expect, it } from "vitest";
import {
  DEFAULT_SIDEBAR_WIDTHS,
  MODULE_SIDEBAR_CONFIG,
  normalizeSidebarWidths,
} from "./sidebar-layout";

describe("sidebar layout normalization", () => {
  it("exposes the module-specific defaults and bounds", () => {
    expect(DEFAULT_SIDEBAR_WIDTHS).toEqual({ api: 320, ssh: 248, database: 280 });
    expect(MODULE_SIDEBAR_CONFIG).toEqual({
      api: { defaultWidth: 320, minWidth: 220, maxWidth: 560 },
      ssh: { defaultWidth: 248, minWidth: 220, maxWidth: 420 },
      database: { defaultWidth: 280, minWidth: 220, maxWidth: 520 },
    });
  });

  it("falls back to module defaults for invalid new values", () => {
    expect(
      normalizeSidebarWidths({ api: null, ssh: "250", database: Number.NaN }),
    ).toEqual({ api: 320, ssh: 248, database: 280 });
  });

  it("clamps a legacy global width independently for each module", () => {
    expect(normalizeSidebarWidths(undefined, 500)).toEqual({
      api: 500,
      ssh: 420,
      database: 500,
    });
  });
});
