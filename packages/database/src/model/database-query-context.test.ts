import { describe, expect, it } from "vitest";
import { buildDatabaseTree } from "./database-tree";
import { normalizeQueryContext } from "./database-query-context";

describe("normalizeQueryContext", () => {
  it("does not select the first MySQL catalog for a new query", () => {
    const tree = buildDatabaseTree([
      { catalog: "analytics", name: "events", kind: "table", columns: [] },
      { catalog: "insur", name: "customer", kind: "table", columns: [] },
    ]);

    expect(normalizeQueryContext({ catalog: null, schema: null }, tree)).toEqual({
      catalog: null,
      schema: null,
    });
  });

  it("uses the connection default only to resolve PostgreSQL schemas", () => {
    const tree = buildDatabaseTree([
      { catalog: "app", schema: "public", name: "users", kind: "table", columns: [] },
      { catalog: "app", schema: "audit", name: "events", kind: "table", columns: [] },
    ]);

    expect(normalizeQueryContext({ catalog: null, schema: null }, tree, "app")).toEqual({
      catalog: null,
      schema: "public",
    });
  });

  it("preserves an explicitly selected catalog", () => {
    const tree = buildDatabaseTree([
      { catalog: "analytics", name: "events", kind: "table", columns: [] },
      { catalog: "insur", name: "customer", kind: "table", columns: [] },
    ]);

    expect(normalizeQueryContext({ catalog: "insur", schema: null }, tree)).toEqual({
      catalog: "insur",
      schema: null,
    });
  });
});
