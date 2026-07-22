import { describe, expect, it } from "vitest";
import {
  resolveExecutableStatements,
  splitSqlStatements,
  statementAtOffset,
} from "./sql-statements";

describe("splitSqlStatements", () => {
  it("splits on semicolons outside quotes and comments", () => {
    const parts = splitSqlStatements("select 1; select 2;");
    expect(parts.map((part) => part.sql)).toEqual(["select 1", "select 2"]);
  });

  it("keeps semicolons inside string literals", () => {
    const parts = splitSqlStatements("select 'a;b'; select 2");
    expect(parts.map((part) => part.sql)).toEqual(["select 'a;b'", "select 2"]);
  });

  it("ignores semicolons in line and block comments", () => {
    const parts = splitSqlStatements("select 1; -- trailing; note\nselect /* ; */ 2");
    expect(parts.map((part) => part.sql)).toEqual([
      "select 1",
      "-- trailing; note\nselect /* ; */ 2",
    ]);
  });

  it("handles escaped quotes", () => {
    const parts = splitSqlStatements("select 'it''s'; select \"a\"\"b\"");
    expect(parts.map((part) => part.sql)).toEqual(["select 'it''s'", 'select "a""b"']);
  });

  it("returns an empty list for whitespace-only input", () => {
    expect(splitSqlStatements("   \n  ;  ")).toEqual([]);
  });
});

describe("statementAtOffset", () => {
  it("returns the statement under the cursor", () => {
    const source = "select 1; select 2; select 3;";
    const second = statementAtOffset(source, source.indexOf("select 2"));
    expect(second?.sql).toBe("select 2");
  });

  it("falls back to the next statement on whitespace between statements", () => {
    const source = "select 1;   select 2;";
    const between = source.indexOf("   ");
    expect(statementAtOffset(source, between)?.sql).toBe("select 2");
  });
});

describe("resolveExecutableStatements", () => {
  it("resolves Run Current from the cursor when no override sql is given", () => {
    const source = "select 1; select 2;";
    expect(
      resolveExecutableStatements(source, {
        mode: "current",
        cursorOffset: source.indexOf("select 2"),
      }),
    ).toEqual(["select 2"]);
  });

  it("resolves Run All from the full buffer", () => {
    expect(resolveExecutableStatements("select 1; select 2;", { mode: "all" })).toEqual([
      "select 1",
      "select 2",
    ]);
  });

  it("splits a multi-statement selection for Run Current", () => {
    expect(
      resolveExecutableStatements("", {
        mode: "current",
        sql: "select 1; select 2;",
      }),
    ).toEqual(["select 1", "select 2"]);
  });
});
