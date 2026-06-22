import type {
  ApiCollection,
  ApiEnvironment,
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

export function normalizeEnvironmentName(name: string) {
  return name.trim().toLowerCase();
}

export function findDuplicateEnvironmentName(
  environments: Array<Pick<ApiEnvironment, "id" | "name">>,
  name: string,
  excludeId?: string,
) {
  const normalized = normalizeEnvironmentName(name);
  if (!normalized) {
    return null;
  }
  return (
    environments.find(
      (environment) =>
        environment.id !== excludeId &&
        normalizeEnvironmentName(environment.name) === normalized,
    )?.name ?? null
  );
}

export function nextEnvironmentName(
  baseName: string,
  environments: Array<Pick<ApiEnvironment, "id" | "name">>,
) {
  const base = baseName.trim() || "New Environment";
  if (!findDuplicateEnvironmentName(environments, base)) {
    return base;
  }

  let suffix = 2;
  while (findDuplicateEnvironmentName(environments, `${base} ${suffix}`)) {
    suffix += 1;
  }
  return `${base} ${suffix}`;
}

export function defaultAuthConfig(): ApiAuthConfig {
  return { type: "none" };
}

export function parseKeyValues(value: unknown): KeyValue[] {
  if (Array.isArray(value)) {
    return sanitizeKeyValues(value);
  }
  if (typeof value === "object" && value !== null) {
    return Object.entries(value).map(([key, itemValue]) => ({
      key,
      value: itemValue == null ? "" : String(itemValue),
      enabled: true,
    }));
  }
  if (typeof value !== "string") {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    return parseKeyValues(parsed);
  } catch {
    return [];
  }
}

export function savedRequestToInput(
  saved: ApiSavedRequest,
  workspaceId: string,
): ApiRequestInput {
  return {
    workspaceId,
    name: saved.name,
    folderPath: saved.folderPath,
    collectionId: saved.collectionId,
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
    folderPath: null,
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

export function parseCollectionImport(
  value: unknown,
  workspaceId: string,
): ApiRequestInput[] {
  const rawItems = Array.isArray(value)
    ? value
    : typeof value === "object" && value !== null && "savedRequests" in value
      ? (value as { savedRequests?: unknown }).savedRequests
      : [];
  if (!Array.isArray(rawItems)) {
    return [];
  }

  return rawItems
    .map((item) => normalizeImportedRequest(item, workspaceId))
    .filter((item): item is ApiRequestInput => item !== null);
}

function normalizeImportedRequest(
  item: unknown,
  workspaceId: string,
): ApiRequestInput | null {
  if (typeof item !== "object" || item === null) {
    return null;
  }
  const candidate = item as Partial<ApiRequestInput>;
  if (typeof candidate.method !== "string" || typeof candidate.url !== "string") {
    return null;
  }

  return {
    workspaceId,
    name: typeof candidate.name === "string" ? candidate.name : undefined,
    folderPath: typeof candidate.folderPath === "string" ? candidate.folderPath : null,
    collectionId:
      typeof candidate.collectionId === "string" ? candidate.collectionId : null,
    method: candidate.method.toUpperCase(),
    url: candidate.url,
    headers: parseKeyValues(candidate.headers),
    query: parseKeyValues(candidate.query),
    body: typeof candidate.body === "string" ? candidate.body : undefined,
    bodyKind: typeof candidate.bodyKind === "string" ? candidate.bodyKind : "json",
    timeoutMs: typeof candidate.timeoutMs === "number" ? candidate.timeoutMs : 60_000,
  };
}

function sanitizeKeyValues(items: unknown[]): KeyValue[] {
  return items
    .map(normalizeKeyValue)
    .filter((item): item is KeyValue => item !== null);
}

function normalizeKeyValue(item: unknown): KeyValue | null {
  if (typeof item !== "object" || item === null) {
    return null;
  }
  const candidate = item as Record<string, unknown>;
  if (typeof candidate.key !== "string") {
    return null;
  }
  return {
    key: candidate.key,
    value: candidate.value == null ? "" : String(candidate.value),
    enabled: typeof candidate.enabled === "boolean" ? candidate.enabled : true,
  };
}

export function groupSavedRequests(items: ApiSavedRequest[]) {
  const groups = new Map<string, ApiSavedRequest[]>();
  for (const item of items) {
    const folder = item.folderPath?.trim() || "Unfiled";
    groups.set(folder, [...(groups.get(folder) ?? []), item]);
  }

  return Array.from(groups.entries())
    .sort(([left], [right]) => {
      if (left === "Unfiled") return -1;
      if (right === "Unfiled") return 1;
      return left.localeCompare(right);
    })
    .map(([folder, groupItems]) => ({
      folder,
      items: groupItems.sort((left, right) => left.name.localeCompare(right.name)),
    }));
}

export type FolderNode = {
  folders: FolderNode[];
  name: string;
  path: string;
  requests: ApiSavedRequest[];
};

export type FolderTree = {
  folders: FolderNode[];
  rootRequests: ApiSavedRequest[];
};

export type ApiCollectionGroup = {
  collection: ApiCollection | null;
  id: string | null;
  name: string;
  tree: FolderTree;
};

/**
 * Build a nested folder tree from `folderPath` segments. `extraFolders` are
 * empty folder paths (persisted on the collection) that should appear even
 * without any saved request. Folderless requests land at the root.
 */
export function buildFolderTree(
  requests: ApiSavedRequest[],
  extraFolders: string[] = [],
): FolderTree {
  const root: FolderNode = { folders: [], name: "", path: "", requests: [] };

  function ensureFolder(rawPath: string): FolderNode {
    const segments = rawPath
      .split("/")
      .map((segment) => segment.trim())
      .filter(Boolean);
    let node = root;
    let accumulated = "";
    for (const segment of segments) {
      accumulated = accumulated ? `${accumulated}/${segment}` : segment;
      let child = node.folders.find((folder) => folder.name === segment);
      if (!child) {
        child = { folders: [], name: segment, path: accumulated, requests: [] };
        node.folders.push(child);
      }
      node = child;
    }
    return node;
  }

  for (const folder of extraFolders) {
    if (folder.trim()) {
      ensureFolder(folder);
    }
  }
  for (const request of requests) {
    const path = request.folderPath?.trim();
    if (path) {
      ensureFolder(path).requests.push(request);
    } else {
      root.requests.push(request);
    }
  }

  sortFolderNode(root);
  return { folders: root.folders, rootRequests: root.requests };
}

function sortFolderNode(node: FolderNode) {
  node.folders.sort((left, right) => left.name.localeCompare(right.name));
  node.requests.sort((left, right) => left.name.localeCompare(right.name));
  for (const child of node.folders) {
    sortFolderNode(child);
  }
}

/** Flatten every request in a folder tree (root + all nested folders). */
export function collectTreeRequests(tree: FolderTree): ApiSavedRequest[] {
  const result = [...tree.rootRequests];
  const walk = (folders: FolderNode[]) => {
    for (const folder of folders) {
      result.push(...folder.requests);
      walk(folder.folders);
    }
  };
  walk(tree.folders);
  return result;
}

/**
 * Group saved requests under their owning collection, then into a nested folder
 * tree (collection-owned empty folders included). Empty collections are still
 * returned so they remain visible. Requests with no collection — or whose
 * collection no longer exists — fall under a synthetic "Unfiled" group first.
 */
export function groupRequestsByCollection(
  requests: ApiSavedRequest[],
  collections: ApiCollection[],
  unfiledLabel: string,
): ApiCollectionGroup[] {
  const byCollection = new Map<string | null, ApiSavedRequest[]>();
  for (const request of requests) {
    const key = request.collectionId ?? null;
    byCollection.set(key, [...(byCollection.get(key) ?? []), request]);
  }

  const known = new Set(collections.map((collection) => collection.id));
  const groups: ApiCollectionGroup[] = [...collections]
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((collection) => ({
      collection,
      id: collection.id,
      name: collection.name,
      tree: buildFolderTree(
        byCollection.get(collection.id) ?? [],
        collection.folders,
      ),
    }));

  const unfiled = requests.filter(
    (request) => !request.collectionId || !known.has(request.collectionId),
  );
  if (unfiled.length) {
    groups.unshift({
      collection: null,
      id: null,
      name: unfiledLabel,
      tree: buildFolderTree(unfiled),
    });
  }

  return groups;
}

export function duplicateEnvironmentKeys(variables: KeyValue[]) {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const variable of variables) {
    const key = variable.key.trim().toLowerCase();
    if (!key || !variable.enabled) {
      continue;
    }
    if (seen.has(key)) {
      duplicates.add(variable.key.trim());
    }
    seen.add(key);
  }
  return Array.from(duplicates);
}

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

export function resolveTemplateLoose(
  value: string,
  variables: KeyValue[],
): string {
  return variables
    .filter((item) => item.enabled && item.key.trim())
    .reduce(
      (current, item) =>
        current.split(`{{${item.key.trim()}}}`).join(item.value),
      value,
    );
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
