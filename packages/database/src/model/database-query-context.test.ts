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

  it("freezes the configured database without guessing the server search path", () => {
    const tree = buildDatabaseTree([
      { catalog: "app", schema: "public", name: "users", kind: "table", columns: [] },
      { catalog: "app", schema: "audit", name: "events", kind: "table", columns: [] },
    ]);

    expect(normalizeQueryContext({ catalog: null, schema: null }, tree, "app")).toEqual({
      catalog: "app",
      schema: null,
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

it("retains explicit unavailable catalog/schema without guessing from the tree", () => {
  expect(normalizeQueryContext({ catalog: "archived", schema: "private" }, { catalogs: [] }, "default"))
    .toEqual({ catalog: "archived", schema: "private" });
});
