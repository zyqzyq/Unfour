import { describe, expect, it } from "vitest";
import { getModuleSwitcherItems } from "./module-helpers";

describe("module switcher items", () => {
  it("keeps API, SSH, and Database in the product navigation order", () => {
    expect(getModuleSwitcherItems().map((item) => item.id)).toEqual([
      "api-main",
      "ssh-main",
      "database-main",
      "flow-main",
    ]);
    expect(getModuleSwitcherItems().map((item) => item.label)).toEqual([
      "API Client",
      "SSH Terminal",
      "Database",
      "Flow",
    ]);
  });
});
