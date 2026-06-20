import { describe, expect, it } from "vitest";
import {
  bodyFieldsFromInput,
  bodyFieldsToInput,
  parseKeyValues,
  savedRequestToInput,
  groupSavedRequests,
  groupRequestsByCollection,
  buildFolderTree,
  collectTreeRequests,
  duplicateEnvironmentKeys,
  headersWithAuthMetadata,
  isSensitiveKey,
  formatByteSize,
  parseCollectionImport,
  queryFromUrl,
  splitAuthMetadata,
  stripUrlQuery,
  syncUrlQuery,
} from "./request-utils";
import type {
  ApiCollection,
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
      folderPath: "",
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

describe("auth metadata helpers", () => {
  it("stores auth metadata without persisted secret values", () => {
    const headers = headersWithAuthMetadata([], {
      type: "bearer",
      token: "secret",
    });
    const { auth, headers: visibleHeaders } = splitAuthMetadata(headers);

    expect(visibleHeaders).toEqual([]);
    expect(auth).toEqual({ type: "bearer", token: "" });
    expect(headers[0].value).not.toContain("secret");
  });
});

describe("savedRequestToInput", () => {
  it("converts a saved request to input form", () => {
    const saved: ApiSavedRequest = {
      id: "req-1",
      workspaceId: "ws-1",
      name: "Get Users",
      folderPath: "Examples / Auth",
      collectionId: null,
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
    expect(result.headers).toHaveLength(1);
    expect(result.query).toEqual([]);
    expect(result.timeoutMs).toBe(60_000);
  });
});

describe("groupSavedRequests", () => {
  it("groups by folderPath with Unfiled first", () => {
    const items: ApiSavedRequest[] = [
      makeSavedRequest("B Request", "Beta"),
      makeSavedRequest("A Request", "Alpha"),
      makeSavedRequest("Unfiled One", ""),
      makeSavedRequest("Unfiled Two", null),
    ];
    const groups = groupSavedRequests(items);
    expect(groups).toHaveLength(3);
    expect(groups[0].folder).toBe("Unfiled");
    expect(groups[0].items).toHaveLength(2);
    expect(groups[1].folder).toBe("Alpha");
    expect(groups[2].folder).toBe("Beta");
  });

  it("sorts items within groups by name", () => {
    const items: ApiSavedRequest[] = [
      makeSavedRequest("Zebra", "Group"),
      makeSavedRequest("Apple", "Group"),
    ];
    const groups = groupSavedRequests(items);
    expect(groups[0].items[0].name).toBe("Apple");
    expect(groups[0].items[1].name).toBe("Zebra");
  });

  it("returns empty for no items", () => {
    expect(groupSavedRequests([])).toEqual([]);
  });
});

describe("buildFolderTree", () => {
  it("nests folders by path segment and keeps folderless requests at root", () => {
    const tree = buildFolderTree([
      makeSavedRequest("Login", "Auth"),
      makeSavedRequest("Refresh", "Auth/Tokens"),
      makeSavedRequest("Root Request", null),
    ]);
    expect(tree.rootRequests.map((r) => r.name)).toEqual(["Root Request"]);
    const auth = tree.folders.find((f) => f.name === "Auth");
    expect(auth?.path).toBe("Auth");
    expect(auth?.requests.map((r) => r.name)).toEqual(["Login"]);
    const tokens = auth?.folders.find((f) => f.name === "Tokens");
    expect(tokens?.path).toBe("Auth/Tokens");
    expect(tokens?.requests.map((r) => r.name)).toEqual(["Refresh"]);
  });

  it("includes empty extra folders that have no requests", () => {
    const tree = buildFolderTree([], ["Drafts", "Auth/Tokens"]);
    expect(tree.folders.map((f) => f.name).sort()).toEqual(["Auth", "Drafts"]);
    const auth = tree.folders.find((f) => f.name === "Auth");
    expect(auth?.folders[0].name).toBe("Tokens");
    expect(collectTreeRequests(tree)).toEqual([]);
  });
});

describe("groupRequestsByCollection", () => {
  const collections: ApiCollection[] = [
    makeCollection("c-1", "Beta"),
    makeCollection("c-2", "Alpha", ["Drafts"]),
  ];

  it("returns collections sorted by name, with empty collections kept", () => {
    const groups = groupRequestsByCollection(
      [makeSavedRequest("In Beta", null, "c-1")],
      collections,
      "Unfiled",
    );
    expect(groups.map((group) => group.name)).toEqual(["Alpha", "Beta"]);
    const alpha = groups.find((group) => group.id === "c-2");
    // Empty collection still surfaces its persisted empty folder.
    expect(alpha?.tree.rootRequests).toEqual([]);
    expect(alpha?.tree.folders[0].name).toBe("Drafts");
    const beta = groups.find((group) => group.id === "c-1");
    expect(beta?.tree.rootRequests[0].name).toBe("In Beta");
  });

  it("places uncollected and orphaned requests under Unfiled first", () => {
    const groups = groupRequestsByCollection(
      [
        makeSavedRequest("No Collection", null, null),
        makeSavedRequest("Orphan", null, "missing-collection"),
        makeSavedRequest("In Beta", "Auth", "c-1"),
      ],
      collections,
      "Unfiled",
    );
    expect(groups[0].id).toBeNull();
    expect(groups[0].name).toBe("Unfiled");
    const unfiledNames = collectTreeRequests(groups[0].tree).map((r) => r.name);
    expect(unfiledNames).toContain("No Collection");
    expect(unfiledNames).toContain("Orphan");
  });

  it("omits Unfiled when every request belongs to a collection", () => {
    const groups = groupRequestsByCollection(
      [makeSavedRequest("In Beta", null, "c-1")],
      collections,
      "Unfiled",
    );
    expect(groups.some((group) => group.id === null)).toBe(false);
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

describe("parseCollectionImport", () => {
  it("parses an array of requests", () => {
    const items = [
      { method: "GET", url: "https://example.com", name: "Test" },
    ];
    const result = parseCollectionImport(items, "ws-1");
    expect(result).toHaveLength(1);
    expect(result[0].method).toBe("GET");
    expect(result[0].workspaceId).toBe("ws-1");
  });

  it("parses object with savedRequests property", () => {
    const input = { savedRequests: [{ method: "POST", url: "https://api.test" }] };
    const result = parseCollectionImport(input, "ws-1");
    expect(result).toHaveLength(1);
    expect(result[0].method).toBe("POST");
  });

  it("filters invalid items", () => {
    const items = [
      { method: "GET", url: "https://valid.com" },
      "not an object",
      { method: 123, url: "bad" },
      null,
      { url: "missing method" },
    ];
    const result = parseCollectionImport(items, "ws-1");
    expect(result).toHaveLength(1);
  });

  it("returns empty for invalid input", () => {
    expect(parseCollectionImport(null, "ws-1")).toEqual([]);
    expect(parseCollectionImport("string", "ws-1")).toEqual([]);
    expect(parseCollectionImport(42, "ws-1")).toEqual([]);
  });
});

function makeSavedRequest(
  name: string,
  folderPath: string | null,
  collectionId: string | null = null,
): ApiSavedRequest {
  return {
    id: `id-${name}`,
    workspaceId: "ws-1",
    name,
    folderPath,
    collectionId,
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
  folders: string[] = [],
): ApiCollection {
  return {
    id,
    workspaceId: "ws-1",
    name,
    description: null,
    folders,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}
