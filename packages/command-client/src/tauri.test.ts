import { describe, expect, it } from "vitest";
import {
  browseDatabaseTable,
  cancelSshReconnect,
  closeSshSession,
  connectSshSession,
  createApiCollection,
  deleteSavedSql,
  executeDatabaseQuery,
  getDatabaseSchema,
  getSshSessionHistory,
  listSavedSql,
  listSavedApiRequests,
  saveApiRequest,
  saveDatabaseConnection,
  saveSavedSql,
  saveSshConnection,
  sendSshInput,
  testDatabaseConnection,
  updateApiRequest,
} from "./tauri";

describe("SSH browser mock lifecycle", () => {
  it("supports the health-state contract and reconnect cancellation", async () => {
    const workspaceId = `mock-health-${crypto.randomUUID()}`;
    const connection = await saveSshConnection({
      workspaceId,
      name: "Mock SSH",
      host: "localhost",
      username: "developer",
      authKind: "password",
      credentialRef: "mock-credential-ref",
    });
    const session = await connectSshSession({
      workspaceId,
      connectionId: connection.id,
    });

    expect(session.status).toBe("connected");
    expect(session.reconnectAttempt).toBe(0);
    await expect(
      sendSshInput({ workspaceId, sessionId: session.sessionId, data: "whoami\n" }),
    ).resolves.toMatchObject({ kind: "output" });

    const cancelled = await cancelSshReconnect({
      workspaceId,
      sessionId: session.sessionId,
    });
    expect(cancelled.status).toBe("disconnected");
    await expect(
      sendSshInput({ workspaceId, sessionId: session.sessionId, data: "pwd\n" }),
    ).rejects.toThrow("not connected");

    const closed = await closeSshSession({
      workspaceId,
      sessionId: session.sessionId,
    });
    expect(closed.status).toBe("disconnected");
  });

  it("hydrates only safe output for the requested workspace and session", async () => {
    const workspaceId = `mock-history-${crypto.randomUUID()}`;
    const connection = await saveSshConnection({
      workspaceId,
      name: "Mock SSH history",
      host: "localhost",
      username: "developer",
      authKind: "password",
      credentialRef: "must-not-be-persisted",
    });
    const session = await connectSshSession({
      workspaceId,
      connectionId: connection.id,
    });
    await sendSshInput({
      workspaceId,
      sessionId: session.sessionId,
      data: "password=secret\n",
    });

    const history = await getSshSessionHistory({
      workspaceId,
      sessionId: session.sessionId,
    });
    expect(history.length).toBeGreaterThan(0);
    expect(history.every((event) => event.kind !== "input")).toBe(true);
    expect(history.map((event) => event.data).join("")).not.toContain("secret");
    expect(history.map((event) => event.data).join("")).not.toContain(
      "must-not-be-persisted",
    );
    await expect(
      getSshSessionHistory({
        workspaceId: `other-${workspaceId}`,
        sessionId: session.sessionId,
      }),
    ).resolves.toEqual([]);
  });
});

describe("API body redaction in browser mock", () => {
  it("redacts sensitive fields in saved request body while preserving structure", async () => {
    const workspaceId = `mock-redaction-${crypto.randomUUID()}`;
    const sensitiveBody = JSON.stringify({
      username: "alice",
      authorization: "Bearer secret-token-123",
      nested: {
        xApiKey: "should-not-redact-different-key",
        "x-api-key": "real-secret-key",
        items: [{ cookie: "session=abc123", name: "item1" }],
      },
    });

    const saved = await saveApiRequest({
      workspaceId,
      name: "Redaction Test",
      method: "POST",
      url: "https://api.example.com/login",
      headers: [
        { key: "Content-Type", value: "application/json", enabled: true },
        { key: "Authorization", value: "Bearer secret-token-123", enabled: true },
      ],
      query: [],
      body: sensitiveBody,
      bodyKind: "json",
    });

    expect(saved.body).not.toBeNull();
    const parsed = JSON.parse(saved.body!);
    // Non-sensitive fields preserved
    expect(parsed.username).toBe("alice");
    expect(parsed.nested.items[0].name).toBe("item1");
    // Sensitive fields redacted
    expect(parsed.authorization).toBe("<redacted>");
    expect(parsed.nested["x-api-key"]).toBe("<redacted>");
    expect(parsed.nested.items[0].cookie).toBe("<redacted>");
    // Non-sensitive key with similar name not redacted
    expect(parsed.nested.xApiKey).toBe("should-not-redact-different-key");

    // Headers also redacted
    const headers = JSON.parse(saved.headersJson);
    const authHeader = headers.find((h: { key: string }) => h.key === "Authorization");
    expect(authHeader.value).toBe("<redacted>");
    const ctHeader = headers.find((h: { key: string }) => h.key === "Content-Type");
    expect(ctHeader.value).toBe("application/json");
  });

  it("preserves non-sensitive JSON body unchanged", async () => {
    const workspaceId = `mock-no-redaction-${crypto.randomUUID()}`;
    const cleanBody = JSON.stringify({ name: "test", count: 42, tags: ["a", "b"] });

    const saved = await saveApiRequest({
      workspaceId,
      name: "Clean Body Test",
      method: "POST",
      url: "https://api.example.com/data",
      headers: [],
      query: [],
      body: cleanBody,
      bodyKind: "json",
    });

    // Body string returned verbatim when no sensitive keys exist
    expect(saved.body).toBe(cleanBody);
  });

  it("handles non-JSON and empty bodies gracefully", async () => {
    const workspaceId = `mock-plain-${crypto.randomUUID()}`;

    const plainSaved = await saveApiRequest({
      workspaceId,
      name: "Plain Text",
      method: "POST",
      url: "https://api.example.com/upload",
      headers: [],
      query: [],
      body: "this is plain text, not json",
      bodyKind: "text",
    });
    expect(plainSaved.body).toBe("this is plain text, not json");

    const emptySaved = await saveApiRequest({
      workspaceId,
      name: "Empty Body",
      method: "GET",
      url: "https://api.example.com/items",
      headers: [],
      query: [],
      body: undefined,
      bodyKind: "none",
    });
    expect(emptySaved.body).toBeNull();
  });

  it("updates an existing saved request without creating a duplicate", async () => {
    const workspaceId = `mock-update-${crypto.randomUUID()}`;
    const collection = await createApiCollection(workspaceId, "Public APIs");
    const saved = await saveApiRequest({
      workspaceId,
      name: "Original",
      collectionId: collection.id,
      method: "GET",
      url: "https://api.example.com/original",
      headers: [],
      query: [],
      body: undefined,
      bodyKind: "none",
      authJson: JSON.stringify({ type: "bearer", token: "{{api_token}}" }),
    });
    expect(saved.authJson).toBe(JSON.stringify({ type: "bearer", token: "{{api_token}}" }));

    const updated = await updateApiRequest(workspaceId, saved.id, {
      workspaceId,
      name: "Updated",
      collectionId: null,
      folderPath: "Moved",
      method: "POST",
      url: "https://api.example.com/updated",
      headers: [],
      query: [],
      body: "{}",
      bodyKind: "json",
      authJson: JSON.stringify({
        type: "api-key",
        addTo: "header",
        key: "X-API-Key",
        value: "{{api_key}}",
      }),
    });

    expect(updated.id).toBe(saved.id);
    expect(updated.name).toBe("Updated");
    expect(updated.collectionId).toBeNull();
    expect(updated.folderPath).toBe("Moved");
    expect(updated.authJson).toBe(
      JSON.stringify({
        type: "api-key",
        addTo: "header",
        key: "X-API-Key",
        value: "{{api_key}}",
      }),
    );
    await expect(listSavedApiRequests(workspaceId)).resolves.toHaveLength(1);
    await expect(
      saveApiRequest({
        workspaceId,
        name: "Bad collection",
        collectionId: "missing",
        method: "GET",
        url: "https://api.example.com/bad",
        headers: [],
        query: [],
        body: undefined,
        bodyKind: "none",
      }),
    ).rejects.toThrow("api collection not found");
  });
});

describe("Database saved SQL browser mock", () => {
  it("saves, updates, lists, and deletes workspace-scoped snippets", async () => {
    const workspaceId = `mock-saved-sql-${crypto.randomUUID()}`;
    const otherWorkspaceId = `mock-saved-sql-other-${crypto.randomUUID()}`;

    const saved = await saveSavedSql({
      workspaceId,
      connectionId: "conn-1",
      name: " Recent users ",
      sql: " SELECT * FROM users ",
    });

    expect(saved.name).toBe("Recent users");
    expect(saved.sql).toBe("SELECT * FROM users");
    await expect(listSavedSql(workspaceId)).resolves.toEqual([saved]);
    await expect(listSavedSql(otherWorkspaceId)).resolves.toEqual([]);

    const updated = await saveSavedSql({
      id: saved.id,
      workspaceId,
      connectionId: null,
      name: "Active users",
      sql: "SELECT * FROM users WHERE active",
    });

    expect(updated.id).toBe(saved.id);
    expect(updated.connectionId).toBeNull();
    expect(updated.createdAt).toBe(saved.createdAt);
    expect(updated.updatedAt >= saved.updatedAt).toBe(true);
    await expect(
      saveSavedSql({
        id: saved.id,
        workspaceId: otherWorkspaceId,
        name: "Wrong workspace",
        sql: "SELECT 1",
      }),
    ).rejects.toThrow("saved SQL not found");

    await expect(deleteSavedSql(workspaceId, saved.id)).resolves.toEqual([]);
    await expect(deleteSavedSql(workspaceId, saved.id)).rejects.toThrow("saved SQL not found");
  });
});

describe("MySQL browser mock compatibility", () => {
  it("supports connection test, schema browsing, read queries, pagination, and confirmation", async () => {
    const workspaceId = `mock-mysql-${crypto.randomUUID()}`;
    const connection = await saveDatabaseConnection({
      workspaceId,
      name: "Mock MySQL",
      driver: "mysql",
      host: "127.0.0.1",
      port: 3306,
      database: "app",
      username: "developer",
      credentialRef: "unfour:mock:database-password:ref",
    });

    await expect(testDatabaseConnection(workspaceId, connection.id)).resolves.toMatchObject({
      ok: true,
      serverVersion: "mock-mysql-8.x",
    });

    const schema = await getDatabaseSchema(workspaceId, connection.id);
    // MySQL exposes each database at the catalog level (no nested schema).
    expect(schema.tables.map((table) => table.catalog)).toEqual(["app", "analytics"]);
    expect(schema.tables.every((table) => table.schema == null)).toBe(true);
    expect(schema.tables[0].columns[0]).toMatchObject({
      name: "id",
      primaryKey: true,
    });

    await expect(
      executeDatabaseQuery({
        workspaceId,
        connectionId: connection.id,
        sql: "SELECT id, email FROM users",
        limit: 25,
      }),
    ).resolves.toMatchObject({
      safety: { classification: "read", requiresConfirmation: false },
    });

    const browse = await browseDatabaseTable({
      workspaceId,
      connectionId: connection.id,
      catalog: "analytics",
      tableName: "events",
      limit: 2,
      offset: 1,
    });
    expect(browse.sql).toBe("SELECT * FROM `analytics`.`events` LIMIT 2 OFFSET 1");
    expect(browse.result.rows).toHaveLength(2);

    await expect(
      executeDatabaseQuery({
        workspaceId,
        connectionId: connection.id,
        sql: "UPDATE users SET active = false",
      }),
    ).rejects.toMatchObject({
      code: "CONFIRMATION_REQUIRED",
    });
  });
});
