import { expect, it } from "vitest";
import type { FlowDefinition } from "@unfour/command-client";
import { apiEnvironmentErrors } from "./apiEnvironmentValidation";
import type { Resources } from "./model";

const resources: Resources = { api: [{ id: "api", name: "API", url: "{{HOST}}/deploy", headersJson: '[{"key":"Authorization","value":"{{DEV_TOKEN}}","enabled":true}]', queryJson: '[]', body: "{{BODY}}", authJson: '{"type":"bearer","token":"{{AUTH}}"}' }], ssh: [], database: [], connections: [] };
const flow = (args: Record<string, unknown> = {}): FlowDefinition => ({ id: "f", workspaceId: "ws", name: "Flow", revision: 1, inputs: [], steps: [{ id: "api", name: "Deploy API", kind: "action", timeoutMs: 1000, next: null, action: { capability: "api", resourceId: "api", connectionId: null, arguments: args } }] });

it("reports static saved URL, headers, query, body and auth per step, preserving exact key case", () => {
  expect(apiEnvironmentErrors(flow(), resources, new Set(["HOST", "BODY", "AUTH", "dev_token"]))).toEqual([{ stepId: "api", name: "Deploy API", variable: "DEV_TOKEN" }]);
  expect(apiEnvironmentErrors(flow(), resources, new Set(["HOST", "BODY", "AUTH", "DEV_TOKEN"]))).toEqual([]);
});

it("uses literal replacements and patches, excludes removed templates and unknown runtime values", () => {
  const args = { url: { $ref: "/inputs/url" }, body: "${/inputs/body} {{STATIC}}", headersPatch: [{ key: "authorization", value: "${/inputs/token}", enabled: true }], queryPatch: [{ key: "key", value: "{{QUERY}}", enabled: true }] };
  expect(apiEnvironmentErrors(flow(args), resources, new Set(["AUTH"])).map((p) => p.variable)).toEqual(["STATIC", "QUERY"]);
  expect(apiEnvironmentErrors(flow({ url: "{{${/inputs/key}}}", body: null, headers: { $ref: "/inputs/headers" } }), resources, new Set(["AUTH"]))).toEqual([]);
});

it("preserves duplicate query occurrence behavior and disabled patch removal", () => {
  const queryResources = { ...resources, api: [{ id: "api", name: "API", queryJson: JSON.stringify([{ key: "x", value: "{{FIRST}}", enabled: true }, { key: "x", value: "{{SECOND}}", enabled: true }]) }] };
  expect(apiEnvironmentErrors(flow({ queryPatch: [{ key: "x", value: "", enabled: false, occurrence: 1 }] }), queryResources, new Set()).map((p) => p.variable)).toEqual(["FIRST"]);
  expect(apiEnvironmentErrors(flow({ queryPatch: [{ key: { $ref: "/inputs/key" }, value: "", enabled: false }] }), queryResources, new Set())).toEqual([]);
  expect(apiEnvironmentErrors(flow({ queryPatch: [{ key: "x", value: "{{DYNAMICALLY_ENABLED}}", enabled: { $ref: "/inputs/enabled" }, occurrence: 1 }] }), queryResources, new Set(["FIRST"]))).toEqual([]);
});
