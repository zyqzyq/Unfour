import { describe, expect, it } from "vitest";
import { databaseConnectionToInput, emptyDatabaseConnectionForm } from "./database-credentials";

describe("emptyDatabaseConnectionForm", () => {
  it("drops connection id and credentialRef for the new workspace", () => {
    expect(emptyDatabaseConnectionForm("ws-2")).toEqual({
      workspaceId: "ws-2",
      name: "",
      driver: "sqlite",
      sqlitePath: "",
    });
  });
});

describe("databaseConnectionToInput", () => {
  it("copies persisted fields into editor input for the current workspace", () => {
    expect(
      databaseConnectionToInput(
        {
          id: "db-a",
          workspaceId: "ws-saved",
          name: "App DB",
          driver: "postgres",
          host: "localhost",
          port: 5432,
          database: "app",
          username: "dev",
          sslMode: "prefer",
          sqlitePath: null,
          credentialRef: "unfour:ws-current:database-password:cred-1",
          readOnly: true,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-02T00:00:00Z",
          deletedAt: null,
          revision: 3,
          syncStatus: "local",
          remoteId: null,
        },
        "ws-current",
      ),
    ).toEqual({
      id: "db-a",
      workspaceId: "ws-current",
      name: "App DB",
      driver: "postgres",
      host: "localhost",
      port: 5432,
      database: "app",
      username: "dev",
      sslMode: "prefer",
      sqlitePath: null,
      credentialRef: "unfour:ws-current:database-password:cred-1",
      readOnly: true,
    });
  });
});
