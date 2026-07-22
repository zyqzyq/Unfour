import type {
  ApiHistoryDetail,
  ApiRequestInput,
  ApiSavedRequest,
  KeyValue,
} from "@unfour/command-client";
import type {
  ApiAuthConfig,
  RequestBodyMode,
  RequestDraft,
  RequestRawBodyType,
} from "./model/types";
import { parseAuthConfigWithSchema, parseKeyValuesWithSchema } from "./adapters/request-schema";

export function findDuplicateRequestName(
  savedRequests: ApiSavedRequest[],
  name: string,
  collectionId: string | null,
  parentFolderId: string | null,
  excludeRequestId?: string,
) {
  const normalized = name.trim().toLowerCase();
  if (!normalized) {
    return null;
  }
  return (
    savedRequests.find(
      (request) =>
        request.id !== excludeRequestId &&
        request.name.trim().toLowerCase() === normalized &&
        request.collectionId === (collectionId ?? null) &&
        request.parentFolderId === (parentFolderId ?? null),
    )?.name ?? null
  );
}

export function defaultAuthConfig(): ApiAuthConfig {
  return { type: "none" };
}

export function parseAuthConfig(value: unknown): ApiAuthConfig {
  return parseAuthConfigWithSchema(value);
}

export function parseKeyValues(value: unknown): KeyValue[] {
  return parseKeyValuesWithSchema(value);
}

export function savedRequestToInput(
  saved: ApiSavedRequest,
  workspaceId: string,
): ApiRequestInput {
  return {
    workspaceId,
    name: saved.name,
    parentFolderId: saved.parentFolderId,
    collectionId: saved.collectionId,
    authJson: saved.authJson,
    method: saved.method,
    url: saved.url,
    headers: parseKeyValues(saved.headersJson),
    query: parseKeyValues(saved.queryJson),
    body: saved.body ?? undefined,
    bodyKind: saved.bodyKind,
    timeoutMs: 60_000,
  };
}

export function historyDetailToInput(history: ApiHistoryDetail): ApiRequestInput {
  return {
    workspaceId: history.workspaceId,
    name: history.name ?? `${history.method} ${history.url}`,
    parentFolderId: null,
    collectionId: null,
    method: history.method,
    url: history.url,
    headers: parseKeyValues(history.requestHeadersJson),
    query: parseKeyValues(history.requestQueryJson),
    body: history.requestBody ?? undefined,
    bodyKind: "json",
    timeoutMs: 60_000,
  };
}

export * from "./request-utils/collection-tree";

export function isSensitiveKey(key: string) {
  return /(token|secret|password|passwd|api[_-]?key|auth|credential)/i.test(key);
}

export function formatByteSize(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const kb = bytes / 1024;
  if (kb < 1024) {
    return `${kb.toFixed(kb >= 10 ? 0 : 1)} KB`;
  }
  const mb = kb / 1024;
  return `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`;
}

export function bodyFieldsFromInput(
  bodyKind: string,
  body?: string,
): {
  body: string;
  bodyMode: RequestBodyMode;
  formBody: KeyValue[];
  rawBodyType: RequestRawBodyType;
} {
  const normalized = bodyKind.trim().toLowerCase();
  if (normalized === "none") {
    return {
      body: "",
      bodyMode: "none",
      formBody: [],
      rawBodyType: "json",
    };
  }
  if (normalized === "text" || normalized === "raw:text") {
    return {
      body: body ?? "",
      bodyMode: "raw",
      formBody: [],
      rawBodyType: "text",
    };
  }
  if (
    normalized === "form-urlencoded" ||
    normalized === "x-www-form-urlencoded" ||
    normalized === "urlencoded"
  ) {
    return {
      body: "",
      bodyMode: "form",
      formBody: parseFormBody(body ?? ""),
      rawBodyType: "json",
    };
  }
  return {
    body: body ?? "",
    bodyMode: "raw",
    formBody: [],
    rawBodyType: "json",
  };
}

export function bodyFieldsToInput(
  draft: RequestDraft,
  purpose: "save" | "send",
): Pick<ApiRequestInput, "body" | "bodyKind"> {
  if (draft.bodyMode === "none") {
    return { body: undefined, bodyKind: "none" };
  }
  if (draft.bodyMode === "form") {
    if (purpose === "save") {
      return {
        body: JSON.stringify(draft.formBody),
        bodyKind: "form-urlencoded",
      };
    }
    const params = new URLSearchParams();
    for (const item of sendableKeyValues(draft.formBody)) {
      params.append(item.key, item.value);
    }
    return { body: params.toString(), bodyKind: "form-urlencoded" };
  }
  return {
    body: draft.body,
    bodyKind: draft.rawBodyType === "text" ? "text" : "json",
  };
}

export function queryFromUrl(rawUrl: string): KeyValue[] {
  const { query } = splitUrlParts(rawUrl);
  if (query === null) {
    return [];
  }
  return Array.from(new URLSearchParams(query).entries()).map(([key, value]) => ({
    enabled: true,
    key,
    value,
  }));
}

export function syncUrlQuery(rawUrl: string, query: KeyValue[]): string {
  const parts = splitUrlParts(rawUrl);
  const params = new URLSearchParams();
  for (const item of sendableKeyValues(query)) {
    params.append(item.key, item.value);
  }
  const nextQuery = params.toString();
  return `${parts.base}${nextQuery ? `?${nextQuery}` : ""}${parts.hash}`;
}

export function stripUrlQuery(rawUrl: string): string {
  const parts = splitUrlParts(rawUrl);
  return `${parts.base}${parts.hash}`;
}

export function reconcileDraftPatch(
  draft: RequestDraft,
  patch: Partial<RequestDraft>,
): RequestDraft {
  const next = { ...draft, ...patch };
  if (typeof patch.url === "string" && patch.query === undefined) {
    return {
      ...next,
      query: queryFromUrl(patch.url),
    };
  }
  if (patch.query !== undefined) {
    return {
      ...next,
      url: syncUrlQuery(typeof patch.url === "string" ? patch.url : draft.url, patch.query),
    };
  }
  return next;
}

export function sendableKeyValues(items: KeyValue[]): KeyValue[] {
  return items
    .filter((item) => item.enabled && item.key.trim())
    .map((item) => ({
      enabled: true,
      key: item.key.trim(),
      value: item.value,
    }));
}

export function hasHeader(headers: KeyValue[], key: string): boolean {
  return headers.some(
    (item) => item.enabled && item.key.trim().toLowerCase() === key.toLowerCase(),
  );
}

export function addHeaderIfMissing(
  headers: KeyValue[],
  key: string,
  value: string,
): KeyValue[] {
  return hasHeader(headers, key)
    ? headers
    : [...headers, { enabled: true, key, value }];
}

export function addQueryIfMissing(
  query: KeyValue[],
  key: string,
  value: string,
): KeyValue[] {
  const normalizedKey = key.trim().toLowerCase();
  if (
    query.some(
      (item) => item.enabled && item.key.trim().toLowerCase() === normalizedKey,
    )
  ) {
    return query;
  }
  return [...query, { enabled: true, key: key.trim(), value }];
}

function parseFormBody(body: string): KeyValue[] {
  const parsed = parseKeyValues(body);
  if (parsed.length || !body.trim()) {
    return parsed;
  }
  return Array.from(new URLSearchParams(body).entries()).map(([key, value]) => ({
    enabled: true,
    key,
    value,
  }));
}

function splitUrlParts(rawUrl: string): {
  base: string;
  hash: string;
  query: string | null;
} {
  const hashIndex = rawUrl.indexOf("#");
  const beforeHash = hashIndex >= 0 ? rawUrl.slice(0, hashIndex) : rawUrl;
  const hash = hashIndex >= 0 ? rawUrl.slice(hashIndex) : "";
  const queryIndex = beforeHash.indexOf("?");
  if (queryIndex < 0) {
    return { base: beforeHash, hash, query: null };
  }
  return {
    base: beforeHash.slice(0, queryIndex),
    hash,
    query: beforeHash.slice(queryIndex + 1),
  };
}

