import { expect, it } from "vitest";
import { variableReference, variablesFor } from "./variables";
import { newStep } from "./model";

it("encodes each pointer segment without losing slash, tilde, empty keys or array indices", () => {
  expect(variableReference(["inputs", "a/b~c"])).toEqual({ $ref: "/inputs/a~1b~0c" });
  expect(variableReference(["steps", "db", "rows", "0", ""])).toEqual({ $ref: "/steps/db/rows/0/" });
});
it("offers inputs and previous outputs with the engine's distinct Poll and Wait Until shapes", () => {
  const before = ["api", "database", "ssh", "waitUntil", "poll", "condition", "wait"].map((kind) => ({ ...newStep(kind, kind), id: kind }));
  const options = variablesFor([{ name: "x", type: "string", secret: false, required: true }], before);
  const refs = options.map((option) => variableReference(option.path).$ref);
  expect(refs).toContain("/inputs/x");
  expect(refs).toContain("/steps/waitUntil/result");
  expect(refs).toContain("/steps/poll/body");
  expect(refs).not.toContain("/steps/poll/result");
  expect(refs).toContain("/steps/database/affectedRows");
  expect(refs).not.toContain("/probe/body");
  expect(variablesFor([], [], before[3]).map((option) => variableReference(option.path).$ref)).toContain("/probe/body");
});
