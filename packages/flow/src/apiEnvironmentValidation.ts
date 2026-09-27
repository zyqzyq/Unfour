import type { FlowDefinition } from "@unfour/command-client";
import type { Resources } from "./model";
import { inheritedPairs, type Pair } from "./actionAuthoring";

const dynamic = (value: unknown) => typeof value !== "string" || value.includes("\0");

// Flow resolves argument values (but not saved request strings) before the API
// resolver. Keep a marker so tokens with runtime-generated keys are deferred.
function staticArguments(value: unknown): unknown {
  if (typeof value === "string") return value.replace(/\$\{[^}]*\}/g, "\0");
  if (!value || typeof value !== "object" || "$ref" in value) return value;
  if (Array.isArray(value)) return value.map(staticArguments);
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, staticArguments(child)]));
}

// Model only statically known patches. A runtime key may replace any inherited
// row, so those rows cannot prove that a variable will be required.
function patchedRows(base: Pair[], patch: unknown, query: boolean): unknown[] {
  if (!Array.isArray(patch)) return [];
  // Unknown keys can also overwrite earlier patches. Defer this field to runtime.
  if (patch.some((row) => !row || dynamic(row.key))) return [];
  let rows: unknown[] = [...base];
  const consumed = new Set<number>();
  for (const row of patch) {
    if (query) {
      const indices = base.flatMap((item, index) => item.key === row.key ? [index] : []);
      const index = row.occurrence === undefined ? indices.find((i) => !consumed.has(i)) : indices[row.occurrence];
      if (index !== undefined) {
        consumed.add(index);
        rows[index] = row.enabled === true ? row : null;
      } else if (row.enabled === true) rows.push(row);
    } else {
      rows = rows.filter((item) => (item as Pair).key.toLowerCase() !== row.key.toLowerCase());
      if (row.enabled === true) rows.push(row);
    }
  }
  return rows;
}

function templates(value: unknown): string[] {
  if (typeof value === "string") {
    // A token spanning a Flow interpolation has no statically known key.
    return [...value.matchAll(/\{\{(.*?)\}\}/gs)].flatMap((match) => {
      const key = match[1].trim();
      return key && !key.includes("\0") ? [key] : [];
    });
  }
  if (!value || typeof value !== "object" || "$ref" in value) return [];
  return Object.values(value).flatMap(templates);
}

export function apiEnvironmentErrors(flow: FlowDefinition, resources: Resources, effectiveKeys: Set<string>) {
  return flow.steps.flatMap((step) => {
    const action = step.kind === "action" ? step.action : step.kind === "poll" || step.kind === "waitUntil" ? step.probe : null;
    if (action?.capability !== "api") return [];
    const saved = resources.api.find((request) => request.id === action.resourceId);
    if (!saved) return [];
    const args = Object.fromEntries(Object.entries(action.arguments).map(([key, value]) => [key, staticArguments(value)]));
    const values: unknown[] = ["url" in args ? args.url : saved.url, "body" in args ? args.body : saved.body, saved.authJson];
    for (const key of ["headers", "query"] as const) {
      const base = inheritedPairs(key === "headers" ? saved.headersJson : saved.queryJson);
      const rows = key in args ? args[key] : `${key}Patch` in args ? patchedRows(base, args[`${key}Patch`], key === "query") : base;
      // Runtime resolution visits disabled saved/replacement pairs too. Disabled
      // patches, unlike those pairs, remove the corresponding inherited row.
      values.push(rows);
    }
    return [...new Set(values.flatMap(templates))].filter((key) => !effectiveKeys.has(key)).map((variable) => ({ stepId: step.id, name: step.name, variable }));
  });
}
