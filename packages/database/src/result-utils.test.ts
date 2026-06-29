import { describe, expect, it } from "vitest";
import {
  serializeDatabaseResult,
  serializeDatabaseResultJson,
  serializeDatabaseRow,
  serializeDatabaseCell,
  tryFormatJson,
  isConfirmationRequired,
  confirmationMessage,
  describeDatabaseError,
  formatDatabaseError,
  formatSql,
  buildPreviewSql,
} from "./result-utils";
import type { DatabaseQueryResult } from "@unfour/command-client";

function makeResult(
  columns: string[],
  rows: (string | null)[][],
): DatabaseQueryResult {
  return {
    columns: columns.map((name) => ({ name, dataType: "TEXT" })),
    rows: rows as string[][],
    affectedRows: 0,
    durationMs: 1,
    safety: { classification: "read", confirmed: true, message: null, requiresConfirmation: false },
  };
}

describe("serializeDatabaseResultJson", () => {
  it("serializes rows as JSON objects keyed by column, preserving null", () => {
    const result = makeResult(["id", "name"], [["1", "Alice"], ["2", null]]);
    expect(JSON.parse(serializeDatabaseResultJson(result))).toEqual([
      { id: "1", name: "Alice" },
      { id: "2", name: null },
    ]);
  });
});

describe("tryFormatJson", () => {
  it("pretty-prints JSON objects and arrays", () => {
    const objectResult = tryFormatJson('{"a":1,"b":[2,3]}');
    expect(objectResult.isJson).toBe(true);
    expect(objectResult.formatted).toBe('{\n  "a": 1,\n  "b": [\n    2,\n    3\n  ]\n}');
  });

  it("leaves non-JSON values untouched", () => {
    expect(tryFormatJson("hello")).toEqual({ formatted: "hello", isJson: false });
    expect(tryFormatJson("{not valid")).toEqual({ formatted: "{not valid", isJson: false });
  });
});

describe("serializeDatabaseResult", () => {
  it("serializes a simple CSV result", () => {
    const result = makeResult(["id", "name"], [["1", "Alice"], ["2", "Bob"]]);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe("id,name\r\n1,Alice\r\n2,Bob");
  });

  it("quotes cells containing delimiters", () => {
    const result = makeResult(["col"], [["hello, world"]]);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe('col\r\n"hello, world"');
  });

  it("escapes double quotes within cells", () => {
    const result = makeResult(["col"], [['say "hello"']]);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe('col\r\n"say ""hello"""');
  });

  it("quotes cells containing newlines", () => {
    const result = makeResult(["col"], [["line1\nline2"]]);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe('col\r\n"line1\nline2"');
  });

  it("handles empty result set", () => {
    const result = makeResult(["id", "name"], []);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe("id,name");
  });

  it("uses tab delimiter for TSV", () => {
    const result = makeResult(["a", "b"], [["1", "2"]]);
    const tsv = serializeDatabaseResult(result, "\t");
    expect(tsv).toBe("a\tb\r\n1\t2");
  });

  it("handles null values in rows", () => {
    const result = makeResult(["col"], [[null]]);
    const csv = serializeDatabaseResult(result, ",");
    expect(csv).toBe("col\r\n");
  });

  it("serializes a single row", () => {
    const result = makeResult(["id", "name"], [["1", "Alice"]]);
    expect(serializeDatabaseRow(result, result.rows[0], "\t")).toBe("1\tAlice");
  });

  it("serializes a single cell with escaping", () => {
    expect(serializeDatabaseCell("hello\tworld", "\t")).toBe('"hello\tworld"');
  });
});

describe("formatSql", () => {
  it("upper-cases keywords and breaks major clauses onto their own lines", () => {
    const formatted = formatSql("select id, name from users where id = 1 order by name");
    expect(formatted).toBe("SELECT\n  id,\n  name\nFROM\n  users\nWHERE\n  id = 1\nORDER BY\n  name");
  });

  it("keeps multi-word joins intact instead of splitting them", () => {
    const formatted = formatSql("select * from a left join b on a.id = b.a_id");
    expect(formatted).toBe("SELECT\n  *\nFROM\n  a\n  LEFT JOIN b ON a.id = b.a_id");
  });

  it("does not rewrite the contents of string literals", () => {
    const formatted = formatSql("select 'from where select' as label from t");
    expect(formatted).toBe("SELECT\n  'from where select' AS label\nFROM\n  t");
  });

  it("preserves numeric literals such as LIMIT 100 OFFSET 0", () => {
    const formatted = formatSql("select * from t limit 100 offset 0");
    expect(formatted).toBe("SELECT\n  *\nFROM\n  t\nLIMIT\n  100\nOFFSET\n  0");
  });

  it("returns blank input unchanged", () => {
    expect(formatSql("   ")).toBe("   ");
  });
});

describe("buildPreviewSql", () => {
  it("builds a paged SELECT for a simple identifier", () => {
    expect(buildPreviewSql("users", 100, 0)).toBe("SELECT * FROM users LIMIT 100 OFFSET 0");
  });

  it("computes the offset from the page index", () => {
    expect(buildPreviewSql("users", 50, 2)).toBe("SELECT * FROM users LIMIT 50 OFFSET 100");
  });

  it("quotes identifiers that are not bare names", () => {
    expect(buildPreviewSql("public.users", 25, 0)).toBe('SELECT * FROM "public.users" LIMIT 25 OFFSET 0');
  });
});

describe("isConfirmationRequired", () => {
  it("returns true for matching error object", () => {
    expect(isConfirmationRequired({ code: "CONFIRMATION_REQUIRED" })).toBe(true);
  });

  it("returns false for other error codes", () => {
    expect(isConfirmationRequired({ code: "OTHER_ERROR" })).toBe(false);
  });

  it("returns false for non-objects", () => {
    expect(isConfirmationRequired(null)).toBe(false);
    expect(isConfirmationRequired(undefined)).toBe(false);
    expect(isConfirmationRequired("string")).toBe(false);
    expect(isConfirmationRequired(42)).toBe(false);
  });
});

describe("confirmationMessage", () => {
  it("includes classification when available", () => {
    const error = { code: "CONFIRMATION_REQUIRED", details: { classification: "write" } };
    const msg = confirmationMessage(error);
    expect(msg).toContain("write");
    expect(msg).toContain("Confirmation required");
  });

  it("returns fallback message when no details", () => {
    const msg = confirmationMessage({ code: "CONFIRMATION_REQUIRED" });
    expect(msg).toContain("Confirmation required");
  });

  it("returns fallback for non-object errors", () => {
    const msg = confirmationMessage(null);
    expect(msg).toContain("Confirmation required");
  });
});

describe("formatDatabaseError", () => {
  it("extracts Error message", () => {
    expect(formatDatabaseError(new Error("connection failed"))).toBe("connection failed");
  });

  it("returns string errors as-is", () => {
    expect(formatDatabaseError("disk full")).toBe("disk full");
  });

  it("returns default for unknown types", () => {
    expect(formatDatabaseError(null)).toBe("Unknown database error");
    expect(formatDatabaseError(42)).toBe("Unknown database error");
    expect(formatDatabaseError(undefined)).toBe("Unknown database error");
  });

  it("extracts serialized AppError objects", () => {
    expect(formatDatabaseError({ code: "DATABASE_ERROR", message: "database error: syntax error" })).toBe(
      "database error: syntax error",
    );
  });
});

describe("describeDatabaseError", () => {
  it("categorizes confirmation errors", () => {
    const description = describeDatabaseError({ code: "CONFIRMATION_REQUIRED", message: "confirmation required" });
    expect(description.category).toBe("confirmation");
    expect(description.title).toBe("Confirmation required");
  });

  it("categorizes syntax errors", () => {
    const description = describeDatabaseError({ code: "DATABASE_ERROR", message: "database error: syntax error near FROM" });
    expect(description.category).toBe("syntax");
  });

  it("preserves technical details", () => {
    const description = describeDatabaseError({ code: "DATABASE_ERROR", message: "connection refused" });
    expect(description.category).toBe("connection");
    expect(description.technicalDetail).toContain("DATABASE_ERROR");
  });
});

