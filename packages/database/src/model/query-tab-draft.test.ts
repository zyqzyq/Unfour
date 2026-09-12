import { describe, expect, it } from "vitest";
import { defaultSql } from "./database-state";
import { queryTabHasUnsavedDraft } from "./query-tab-draft";

describe("queryTabHasUnsavedDraft", () => {
  it("allows closing empty or whitespace-only SQL", () => {
    expect(queryTabHasUnsavedDraft({ sql: "" })).toBe(false);
    expect(queryTabHasUnsavedDraft({ sql: "   \n" })).toBe(false);
  });

  it("does not treat unmodified default SQL as a draft", () => {
    expect(queryTabHasUnsavedDraft({ sql: defaultSql, sqlBaseline: defaultSql })).toBe(false);
  });

  it("does not treat SQL that still matches the tab baseline as a draft", () => {
    expect(
      queryTabHasUnsavedDraft({
        sql: "SELECT * FROM users;",
        sqlBaseline: "SELECT * FROM users;",
      }),
    ).toBe(false);
    expect(
      queryTabHasUnsavedDraft({
        sql: "  SELECT * FROM users;  \n",
        sqlBaseline: "SELECT * FROM users;",
      }),
    ).toBe(false);
  });

  it("flags user-authored SQL that diverged from the baseline", () => {
    expect(queryTabHasUnsavedDraft({ sql: "SELECT 1;", sqlBaseline: defaultSql })).toBe(true);
    expect(
      queryTabHasUnsavedDraft({
        sql: "SELECT 2;",
        sqlBaseline: "SELECT 1;",
      }),
    ).toBe(true);
  });

  it("becomes clean after a successful save updates the baseline", () => {
    expect(
      queryTabHasUnsavedDraft({
        sql: "SELECT * FROM users;",
        sqlBaseline: "",
      }),
    ).toBe(true);
    expect(
      queryTabHasUnsavedDraft({
        sql: "SELECT * FROM users;",
        sqlBaseline: "SELECT * FROM users;",
      }),
    ).toBe(false);
    expect(
      queryTabHasUnsavedDraft({
        sql: "SELECT * FROM orders;",
        sqlBaseline: "SELECT * FROM users;",
      }),
    ).toBe(true);
  });

  it("falls back to default SQL when a baseline is missing", () => {
    expect(queryTabHasUnsavedDraft({ sql: defaultSql })).toBe(false);
    expect(queryTabHasUnsavedDraft({ sql: "SELECT 1;" })).toBe(defaultSql.trim() !== "SELECT 1;");
  });
});
