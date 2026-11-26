import { mockStore } from "./state";
import type { ApiRequestInput, CredentialMetadata, KeyValue } from "../../types";

export function resolveInput(input: ApiRequestInput, variables: KeyValue[]) {
  return {
    ...input,
    url: resolveTemplate(input.url, variables),
    headers: input.headers.map((item) => ({
      ...item,
      key: resolveTemplate(item.key, variables),
      value: resolveTemplate(item.value, variables),
    })),
    query: input.query.map((item) => ({
      ...item,
      key: resolveTemplate(item.key, variables),
      value: resolveTemplate(item.value, variables),
    })),
    body: input.body ? resolveTemplate(input.body, variables) : input.body,
  };
}

export function resolveTemplate(value: string, variables: KeyValue[]) {
  return variables
    .filter((item) => item.enabled && item.key)
    .reduce(
      (current, item) => current.split(`{{${item.key}}}`).join(item.value),
      value,
    );
}

export function redactHeaders(headers: ApiRequestInput["headers"]) {
  const sensitive = new Set([
    "authorization",
    "cookie",
    "proxy-authorization",
    "x-api-key",
    "x-auth-token",
  ]);
  return headers.map((item) => ({
    ...item,
    value: sensitive.has(item.key.toLowerCase()) ? "<redacted>" : item.value,
  }));
}

export function redactJsonBody(body: string | null | undefined): string | null {
  if (!body) return null;
  try {
    const parsed = JSON.parse(body);
    const sensitive = new Set([
      "authorization",
      "cookie",
      "proxy-authorization",
      "x-api-key",
      "x-auth-token",
    ]);
    let changed = false;
    function walk(value: unknown): unknown {
      if (value && typeof value === "object" && !Array.isArray(value)) {
        const obj = value as Record<string, unknown>;
        const result: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(obj)) {
          if (sensitive.has(k.toLowerCase())) {
            result[k] = "<redacted>";
            changed = true;
          } else {
            result[k] = walk(v);
          }
        }
        return result;
      }
      if (Array.isArray(value)) {
        return value.map(walk);
      }
      return value;
    }
    const redacted = walk(parsed);
    return changed ? JSON.stringify(redacted) : body;
  } catch {
    return body;
  }
}

export function redactSshLog(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => {
      const lower = line.toLowerCase();
      return [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "x-auth-token",
        "password",
        "passphrase",
      ].some((needle) => lower.includes(needle))
        ? "<redacted>"
        : line;
    })
    .join("\n");
}

export function inspectMockCredential(
  workspaceId: string,
  credentialRef: string,
): CredentialMetadata {
  const [serviceName, refWorkspaceId, kind, recordId] = credentialRef.split(":");
  if (
    serviceName !== "unfour" ||
    refWorkspaceId !== workspaceId ||
    !kind ||
    !recordId ||
    !(credentialRef in mockStore.credentials)
  ) {
    throw new Error("credential not found");
  }

  return {
    workspaceId,
    kind,
    label: "Credential reference",
    credentialRef,
  };
}

export function quoteMySqlIdentifier(value: string) {
  return `\`${value.split("`").join("``")}\``;
}
