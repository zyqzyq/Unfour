import { describe, expect, it } from "vitest";
import {
  SQL_RESULT_ROW_LIMIT_DEFAULT,
  SQL_RESULT_ROW_LIMIT_MAX,
  isSqlResultRowLimitOption,
  sqlResultRowLimit,
} from "./result-row-limit";

describe("sqlResultRowLimit", () => {
  it("defaults when the tab has no explicit cap", () => {
    expect(sqlResultRowLimit(undefined)).toBe(SQL_RESULT_ROW_LIMIT_DEFAULT);
    expect(sqlResultRowLimit(null)).toBe(SQL_RESULT_ROW_LIMIT_DEFAULT);
    expect(sqlResultRowLimit(1.5)).toBe(SQL_RESULT_ROW_LIMIT_DEFAULT);
  });

  it("clamps to the backend range instead of inventing a new cap", () => {
    expect(sqlResultRowLimit(0)).toBe(1);
    expect(sqlResultRowLimit(500)).toBe(500);
    expect(sqlResultRowLimit(SQL_RESULT_ROW_LIMIT_MAX + 50)).toBe(SQL_RESULT_ROW_LIMIT_MAX);
    expect(isSqlResultRowLimitOption(500)).toBe(true);
    expect(isSqlResultRowLimitOption(250)).toBe(false);
  });
});
