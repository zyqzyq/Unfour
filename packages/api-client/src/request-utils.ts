import type {
  ApiCollection,
  ApiCollectionFolder,
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
import { parseAuthConfigWithSchema, parseKeyValuesWithSchema } from "./adapters/request-schema";

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
    parentFolderId: null,
    collectionId: null,
    authJson: typeof candidate.authJson === "string" ? candidate.authJson : undefined,
    method: candidate.method.toUpperCase(),
    url: candidate.url,
    headers: parseKeyValues(candidate.headers),
    query: parseKeyValues(candidate.query),
    body: typeof candidate.body === "string" ? candidate.body : undefined,
    bodyKind: typeof candidate.bodyKind === "string" ? candidate.bodyKind : "json",
    timeoutMs: typeof candidate.timeoutMs === "number" ? candidate.timeoutMs : 60_000,
  };
}

export type FolderNode = {
  collectionId: string;
  folders: FolderNode[];
  id: string;
  name: string;
  parentFolderId: string | null;
  requests: ApiSavedRequest[];
  sortOrder: number;
};

export type FolderTree = {
  folders: FolderNode[];
  rootRequests: ApiSavedRequest[];
};

export type ApiCollectionGroup = {
  collection: ApiCollection;
  id: string;
  name: string;
  tree: FolderTree;
};

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
 * Build the visible API collection tree from persisted folder rows and request
 * parent ids. Empty collections and empty folders remain visible.
 */
export function buildApiCollectionTree(
  collections: ApiCollection[],
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
): ApiCollectionGroup[] {
  const byCollection = new Map<string, ApiSavedRequest[]>();
  const collectionIds = new Set(collections.map((collection) => collection.id));
  for (const request of requests) {
    const key = collectionIds.has(request.collectionId)
      ? request.collectionId
      : collections[0]?.id ?? "";
    byCollection.set(key, [...(byCollection.get(key) ?? []), request]);
  }

  const byCollectionFolders = new Map<string, ApiCollectionFolder[]>();
  for (const folder of folders) {
    if (!collectionIds.has(folder.collectionId)) {
      continue;
    }
    byCollectionFolders.set(folder.collectionId, [
      ...(byCollectionFolders.get(folder.collectionId) ?? []),
      folder,
    ]);
  }

  return [...collections]
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((collection) => ({
      collection,
      id: collection.id,
      name: collection.name,
      tree: buildCollectionFolderTree(
        byCollectionFolders.get(collection.id) ?? [],
        byCollection.get(collection.id) ?? [],
      ),
    }));
}

export function groupRequestsByCollection(
  requests: ApiSavedRequest[],
  collections: ApiCollection[],
): ApiCollectionGroup[] {
  return buildApiCollectionTree(collections, [], requests);
}

function buildCollectionFolderTree(
  folders: ApiCollectionFolder[],
  requests: ApiSavedRequest[],
): FolderTree {
  const folderById = new Map(folders.map((folder) => [folder.id, folder]));
  const nodeById = new Map<string, FolderNode>();
  for (const folder of folders) {
    nodeById.set(folder.id, {
      collectionId: folder.collectionId,
      folders: [],
      id: folder.id,
      name: folder.name,
      parentFolderId: folder.parentFolderId,
      requests: [],
      sortOrder: folder.sortOrder,
    });
  }

  const root: FolderTree = { folders: [], rootRequests: [] };
  for (const folder of folders) {
    const node = nodeById.get(folder.id);
    if (!node) continue;
    const parent = folder.parentFolderId
      ? nodeById.get(folder.parentFolderId)
      : null;
    if (
      parent &&
      parent.collectionId === folder.collectionId &&
      !hasAncestor(folderById, folder.parentFolderId, folder.id)
    ) {
      parent.folders.push(node);
    } else {
      root.folders.push(node);
    }
  }

  for (const request of requests) {
    const parent = request.parentFolderId
      ? nodeById.get(request.parentFolderId)
      : null;
    if (parent && parent.collectionId === request.collectionId) {
      parent.requests.push(request);
    } else {
      root.rootRequests.push(request);
    }
  }

  sortTree(root);
  return root;
}

function hasAncestor(
  folderById: Map<string, ApiCollectionFolder>,
  parentFolderId: string | null,
  targetId: string,
) {
  let current = parentFolderId;
  const seen = new Set<string>();
  while (current) {
    if (current === targetId) return true;
    if (seen.has(current)) return true;
    seen.add(current);
    current = folderById.get(current)?.parentFolderId ?? null;
  }
  return false;
}

function sortTree(tree: FolderTree) {
  tree.folders.sort(compareFolderNodes);
  tree.rootRequests.sort(compareRequests);
  for (const folder of tree.folders) {
    sortFolderNode(folder);
  }
}

function sortFolderNode(node: FolderNode) {
  node.folders.sort(compareFolderNodes);
  node.requests.sort(compareRequests);
  for (const child of node.folders) {
    sortFolderNode(child);
  }
}

function compareFolderNodes(left: FolderNode, right: FolderNode) {
  return left.sortOrder - right.sortOrder || left.name.localeCompare(right.name);
}

function compareRequests(left: ApiSavedRequest, right: ApiSavedRequest) {
  return left.sortOrder - right.sortOrder || left.name.localeCompare(right.name);
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

