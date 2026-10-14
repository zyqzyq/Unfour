/** Backend `DatabaseQueryInput.limit` is clamped to this range. */
export const SQL_RESULT_ROW_LIMIT_MIN = 1;
export const SQL_RESULT_ROW_LIMIT_MAX = 1000;
export const SQL_RESULT_ROW_LIMIT_DEFAULT = 100;
export const SQL_RESULT_ROW_LIMIT_OPTIONS = [100, 500, 1000] as const;

export type SqlResultRowLimitOption = (typeof SQL_RESULT_ROW_LIMIT_OPTIONS)[number];

export function isSqlResultRowLimitOption(value: number): value is SqlResultRowLimitOption {
  return (SQL_RESULT_ROW_LIMIT_OPTIONS as readonly number[]).includes(value);
}

/** Reuse the existing backend cap. Values outside 1..=1000 are clamped. */
export function sqlResultRowLimit(value: number | null | undefined): number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    return SQL_RESULT_ROW_LIMIT_DEFAULT;
  }
  if (value < SQL_RESULT_ROW_LIMIT_MIN) {
    return SQL_RESULT_ROW_LIMIT_MIN;
  }
  if (value > SQL_RESULT_ROW_LIMIT_MAX) {
    return SQL_RESULT_ROW_LIMIT_MAX;
  }
  return value;
}
