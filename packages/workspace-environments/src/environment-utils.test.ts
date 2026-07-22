import { describe, expect, it } from "vitest";
import { findDuplicateEnvironmentName, nextEnvironmentName } from "./environment-utils";

const environments = [
  { id: "env-1", name: "Dev" },
  { id: "env-2", name: "New Environment" },
  { id: "env-3", name: "New Environment 2" },
];

describe("workspace environment name helpers", () => {
  it("detects duplicate names case-insensitively while allowing the current row", () => {
    expect(findDuplicateEnvironmentName(environments, " dev ")).toBe("Dev");
    expect(findDuplicateEnvironmentName(environments, "dev", "env-1")).toBeNull();
  });

  it("generates the next available name", () => {
    expect(nextEnvironmentName("New Environment", environments)).toBe(
      "New Environment 3",
    );
  });
});
