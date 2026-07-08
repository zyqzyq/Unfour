import { describe, expect, it } from "vitest";
import {
  bodyFieldsFromInput,
  bodyFieldsToInput,
  parseKeyValues,
  savedRequestToInput,
  buildApiCollectionTree,
  collectTreeRequests,
  duplicateEnvironmentKeys,
  findDuplicateEnvironmentName,
  isSensitiveKey,
  formatByteSize,
  nextEnvironmentName,
  queryFromUrl,
  stripUrlQuery,
  syncUrlQuery,
} from "./request-utils";
import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiEnvironment,
  ApiSavedRequest,
  KeyValue,
} from "@unfour/command-client";

describe("parseKeyValues", () => {
  it("parses valid JSON array", () => {
    const input = JSON.stringify([
      { key: "Content-Type", value: "application/json", enabled: true },
    ]);
    const result = parseKeyValues(input);
    expect(result).toHaveLength(1);
    expect(result[0].key).toBe("Content-Type");
  });

  it("returns empty array for invalid JSON", () => {
    expect(parseKeyValues("not json")).toEqual([]);
    expect(parseKeyValues("")).toEqual([]);
    expect(parseKeyValues("{")).toEqual([]);
  });

  it("normalizes valid key rows and filters malformed items", () => {
    const input = JSON.stringify([
      { key: "valid", value: "ok", enabled: true },
      { key: "no-value", enabled: true },
      "not an object",
      42,
      null,
    ]);
    const result = parseKeyValues(input);
    expect(result).toHaveLength(2);
    expect(result[0].key).toBe("valid");
    expect(result[1]).toEqual({ key: "no-value", value: "", enabled: true });
  });

  it("converts JSON objects to key-value rows", () => {
    expect(parseKeyValues('{"key":"val"}')).toEqual([
      { key: "key", value: "val", enabled: true },
    ]);
    expect(parseKeyValues('"string"')).toEqual([]);
    expect(parseKeyValues("123")).toEqual([]);
  });
});

describe("URL query helpers", () => {
  it("parses query params from URL while preserving duplicate keys", () => {
    expect(queryFromUrl("https://api.example.com/users?page=1&page=2#top")).toEqual([
      { key: "page", value: "1", enabled: true },
      { key: "page", value: "2", enabled: true },
    ]);
  });

  it("syncs enabled params into the URL and preserves hash", () => {
    const url = syncUrlQuery("https://api.example.com/users?old=1#top", [
      { key: "page", value: "1", enabled: true },
      { key: "debug", value: "1", enabled: false },
      { key: "", value: "skip", enabled: true },
    ]);

    expect(url).toBe("https://api.example.com/users?page=1#top");
  });

  it("strips query before sending to avoid duplicate backend appends", () => {
    expect(stripUrlQuery("https://api.example.com/users?page=1#top")).toBe(
      "https://api.example.com/users#top",
    );
  });
});

describe("body field helpers", () => {
  it("restores form-url-encoded fields from saved JSON body", () => {
    const result = bodyFieldsFromInput(
      "form-urlencoded",
      JSON.stringify([{ key: "a", value: "1", enabled: true }]),
    );

    expect(result.bodyMode).toBe("form");
    expect(result.formBody).toEqual([{ key: "a", value: "1", enabled: true }]);
  });

  it("serializes form rows differently for save and send", () => {
    const draft = {
      auth: { type: "none" as const },
      body: "",
      bodyMode: "form" as const,
      collectionId: null,
      envVariables: [],
      parentFolderId: null,
      formBody: [
        { key: "a", value: "1", enabled: true },
        { key: "b", value: "2", enabled: false },
      ],
      headers: [],
      method: "POST",
      name: "Form",
      query: [],
      rawBodyType: "json" as const,
      url: "https://example.com",
    };

    expect(bodyFieldsToInput(draft, "send")).toEqual({
      body: "a=1",
      bodyKind: "form-urlencoded",
    });
    expect(bodyFieldsToInput(draft, "save").body).toContain('"key":"a"');
  });
});

describe("savedRequestToInput", () => {
  it("converts a saved request to input form", () => {
    const saved: ApiSavedRequest = {
      id: "req-1",
      workspaceId: "ws-1",
      name: "Get Users",
      collectionId: "c-1",
      parentFolderId: "folder-auth",
      sortOrder: 0,
      method: "GET",
      url: "https://api.example.com/users",
      headersJson: JSON.stringify([
        { key: "Authorization", value: "Bearer token", enabled: true },
      ]),
      queryJson: "[]",
      body: null,
      bodyKind: "json",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
      deletedAt: null,
      revision: 1,
      syncStatus: "local",
      remoteId: null,
    };
    const result = savedRequestToInput(saved, "ws-2");
    expect(result.workspaceId).toBe("ws-2");
    expect(result.name).toBe("Get Users");
    expect(result.method).toBe("GET");
    expect(result.collectionId).toBe("c-1");
    expect(result.parentFolderId).toBe("folder-auth");
    expect(result.headers).toHaveLength(1);
    expect(result.query).toEqual([]);
    expect(result.timeoutMs).toBe(60_000);
  });
});

describe("buildApiCollectionTree", () => {
  const collections: ApiCollection[] = [
    makeCollection("c-1", "Beta"),
    makeCollection("c-2", "Alpha"),
  ];

  it("returns collections sorted by name, with empty collections kept", () => {
    const groups = buildApiCollectionTree(
      collections,
      [makeFolder("folder-drafts", "c-2", null, "Drafts")],
      [makeSavedRequest("In Beta", "c-1", null)],
    );
    expect(groups.map((group) => group.name)).toEqual(["Alpha", "Beta"]);
    const alpha = groups.find((group) => group.id === "c-2");
    expect(alpha?.tree.rootRequests).toEqual([]);
    expect(alpha?.tree.folders[0].name).toBe("Drafts");
    const beta = groups.find((group) => group.id === "c-1");
    expect(beta?.tree.rootRequests[0].name).toBe("In Beta");
  });

  it("builds folder rows by parentFolderId and keeps root requests at the collection root", () => {
    const groups = buildApiCollectionTree(
      collections,
      [
        makeFolder("folder-auth", "c-1", null, "Auth"),
        makeFolder("folder-tokens", "c-1", "folder-auth", "Tokens"),
      ],
      [
        makeSavedRequest("Login", "c-1", "folder-auth"),
        makeSavedRequest("Refresh", "c-1", "folder-tokens"),
        makeSavedRequest("Root Request", "c-1", null),
      ],
    );

    const beta = groups.find((group) => group.id === "c-1");
    expect(beta?.tree.rootRequests.map((r) => r.name)).toEqual(["Root Request"]);
    const auth = beta?.tree.folders.find((folder) => folder.id === "folder-auth");
    expect(auth?.requests.map((r) => r.name)).toEqual(["Login"]);
    expect(auth?.folders[0]).toMatchObject({
      id: "folder-tokens",
      name: "Tokens",
    });
    expect(auth?.folders[0].requests.map((r) => r.name)).toEqual(["Refresh"]);
  });

  it("keeps empty folders visible without relying on collection folders_json", () => {
    const groups = buildApiCollectionTree(
      collections,
      [
        makeFolder("folder-auth", "c-1", null, "Auth"),
        makeFolder("folder-tokens", "c-1", "folder-auth", "Tokens"),
      ],
      [],
    );

    const auth = groups
      .find((group) => group.id === "c-1")
      ?.tree.folders.find((folder) => folder.id === "folder-auth");
    expect(auth?.folders[0].name).toBe("Tokens");
    expect(collectTreeRequests(groups[1].tree)).toEqual([]);
  });

  it("sorts sibling folders and sibling requests by sortOrder with folders first", () => {
    const groups = buildApiCollectionTree(
      [makeCollection("c-1", "Only")],
      [
        makeFolder("folder-b", "c-1", null, "B Folder", 20),
        makeFolder("folder-a", "c-1", null, "A Folder", 10),
      ],
      [
        makeSavedRequest("B Request", "c-1", null, 20),
        makeSavedRequest("A Request", "c-1", null, 10),
      ],
    );

    const only = groups[0].tree;
    expect(only.folders.map((folder) => folder.name)).toEqual([
      "A Folder",
      "B Folder",
    ]);
    expect(only.rootRequests.map((request) => request.name)).toEqual([
      "A Request",
      "B Request",
    ]);
    expect([
      ...only.folders.map((folder) => `folder:${folder.name}`),
      ...only.rootRequests.map((request) => `request:${request.name}`),
    ]).toEqual([
      "folder:A Folder",
      "folder:B Folder",
      "request:A Request",
      "request:B Request",
    ]);
  });

  it("ignores folderPath-shaped imported data when building the tree", () => {
    const request = {
      ...makeSavedRequest("Root Request", "c-1", null),
      folderPath: "Legacy/Path",
    } as ApiSavedRequest & { folderPath: string };
    const groups = buildApiCollectionTree(
      [makeCollection("c-1", "Only")],
      [],
      [request],
    );

    expect(groups[0].tree.folders).toEqual([]);
    expect(groups[0].tree.rootRequests[0].name).toBe("Root Request");
  });
});

describe("duplicateEnvironmentKeys", () => {
  it("finds case-insensitive duplicate keys", () => {
    const vars: KeyValue[] = [
      { key: "API_URL", value: "a", enabled: true },
      { key: "api_url", value: "b", enabled: true },
      { key: "OTHER", value: "c", enabled: true },
    ];
    const dupes = duplicateEnvironmentKeys(vars);
    expect(dupes).toHaveLength(1);
    expect(dupes[0].toLowerCase()).toBe("api_url");
  });

  it("ignores disabled and empty keys", () => {
    const vars: KeyValue[] = [
      { key: "DUP", value: "a", enabled: true },
      { key: "dup", value: "b", enabled: false },
      { key: "", value: "c", enabled: true },
    ];
    expect(duplicateEnvironmentKeys(vars)).toEqual([]);
  });

  it("returns empty for no duplicates", () => {
    const vars: KeyValue[] = [
      { key: "A", value: "1", enabled: true },
      { key: "B", value: "2", enabled: true },
    ];
    expect(duplicateEnvironmentKeys(vars)).toEqual([]);
  });
});

describe("environment name helpers", () => {
  it("detects duplicate environment names inside one workspace list", () => {
    const environments: Array<Pick<ApiEnvironment, "id" | "name">> = [
      { id: "env-1", name: "Dev" },
      { id: "env-2", name: "Prod" },
    ];

    expect(findDuplicateEnvironmentName(environments, " dev ")).toBe("Dev");
    expect(findDuplicateEnvironmentName(environments, "dev", "env-1")).toBeNull();
    expect(findDuplicateEnvironmentName(environments, "Stage")).toBeNull();
  });

  it("generates the next available default environment name", () => {
    const environments: Array<Pick<ApiEnvironment, "id" | "name">> = [
      { id: "env-1", name: "New Environment" },
      { id: "env-2", name: "New Environment 2" },
    ];

    expect(nextEnvironmentName("New Environment", environments)).toBe(
      "New Environment 3",
    );
  });
});

describe("isSensitiveKey", () => {
  it("matches sensitive patterns", () => {
    expect(isSensitiveKey("authorization")).toBe(true);
    expect(isSensitiveKey("x-api-key")).toBe(true);
    expect(isSensitiveKey("Authorization")).toBe(true);
    expect(isSensitiveKey("secret_key")).toBe(true);
    expect(isSensitiveKey("password")).toBe(true);
    expect(isSensitiveKey("auth_token")).toBe(true);
    expect(isSensitiveKey("credential")).toBe(true);
  });

  it("does not match non-sensitive keys", () => {
    expect(isSensitiveKey("Content-Type")).toBe(false);
    expect(isSensitiveKey("Accept")).toBe(false);
    expect(isSensitiveKey("base_url")).toBe(false);
  });
});

describe("formatByteSize", () => {
  it("formats bytes correctly", () => {
    expect(formatByteSize(0)).toBe("0 B");
    expect(formatByteSize(512)).toBe("512 B");
    expect(formatByteSize(1023)).toBe("1023 B");
  });

  it("formats kilobytes", () => {
    expect(formatByteSize(1024)).toBe("1.0 KB");
    expect(formatByteSize(10240)).toBe("10 KB");
    expect(formatByteSize(51200)).toBe("50 KB");
  });

  it("formats megabytes", () => {
    expect(formatByteSize(1048576)).toBe("1.0 MB");
    expect(formatByteSize(10485760)).toBe("10 MB");
  });
});

function makeSavedRequest(
  name: string,
  collectionId: string,
  parentFolderId: string | null,
  sortOrder = 0,
): ApiSavedRequest {
  return {
    id: `id-${name}`,
    workspaceId: "ws-1",
    name,
    collectionId,
    parentFolderId,
    sortOrder,
    method: "GET",
    url: "https://example.com",
    headersJson: "[]",
    queryJson: "[]",
    body: null,
    bodyKind: "json",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}

function makeCollection(
  id: string,
  name: string,
): ApiCollection {
  return {
    id,
    workspaceId: "ws-1",
    name,
    description: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

function makeFolder(
  id: string,
  collectionId: string,
  parentFolderId: string | null,
  name: string,
  sortOrder = 0,
): ApiCollectionFolder {
  return {
    id,
    workspaceId: "ws-1",
    collectionId,
    parentFolderId,
    name,
    sortOrder,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
  };
}
