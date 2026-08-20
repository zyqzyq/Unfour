import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DatabaseConnectionInput } from "@unfour/command-client";

vi.mock("@unfour/command-client", () => ({
  createCredential: vi.fn(),
  rotateCredential: vi.fn(),
}));

import { createCredential, rotateCredential } from "@unfour/command-client";
import {
  DATABASE_PASSWORD_KIND,
  emptyDatabaseConnectionForm,
  isCredentialWorkspaceMismatch,
  persistDatabaseConnectionPassword,
} from "./database-credentials";

const createMock = vi.mocked(createCredential);
const rotateMock = vi.mocked(rotateCredential);

const postgresInput: DatabaseConnectionInput = {
  workspaceId: "ws-current",
  name: "App DB",
  driver: "postgres",
  credentialRef: "unfour:ws-other:database-password:cred-1",
};

beforeEach(() => {
  vi.clearAllMocks();
  createMock.mockResolvedValue({
    workspaceId: "ws-current",
    kind: DATABASE_PASSWORD_KIND,
    label: "App DB",
    credentialRef: "unfour:ws-current:database-password:new-cred",
  });
});

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

describe("isCredentialWorkspaceMismatch", () => {
  it("detects SecretStore validation objects and Error wrappers", () => {
    expect(
      isCredentialWorkspaceMismatch({
        code: "VALIDATION_ERROR",
        message: "validation error: credential reference does not belong to the workspace",
      }),
    ).toBe(true);
    expect(
      isCredentialWorkspaceMismatch(
        new Error("validation error: credential reference does not belong to the workspace"),
      ),
    ).toBe(true);
    expect(isCredentialWorkspaceMismatch(new Error("credential secret cannot be empty"))).toBe(
      false,
    );
  });
});

describe("persistDatabaseConnectionPassword", () => {
  it("creates a database-password credential for a new connection", async () => {
    const credentialRef = await persistDatabaseConnectionPassword({
      input: { ...postgresInput, credentialRef: null },
      secret: "secret",
      workspaceId: "ws-current",
    });

    expect(rotateMock).not.toHaveBeenCalled();
    expect(createMock).toHaveBeenCalledWith({
      workspaceId: "ws-current",
      kind: DATABASE_PASSWORD_KIND,
      label: "App DB",
      secret: "secret",
    });
    expect(credentialRef).toBe("unfour:ws-current:database-password:new-cred");
  });

  it("rotates an existing credential in the same workspace", async () => {
    const existing = "unfour:ws-current:database-password:cred-1";
    rotateMock.mockResolvedValue({
      workspaceId: "ws-current",
      kind: DATABASE_PASSWORD_KIND,
      label: "Rotated credential",
      credentialRef: existing,
    });

    const credentialRef = await persistDatabaseConnectionPassword({
      input: { ...postgresInput, credentialRef: existing },
      secret: "rotated",
      workspaceId: "ws-current",
    });

    expect(rotateMock).toHaveBeenCalledWith({
      workspaceId: "ws-current",
      credentialRef: existing,
      secret: "rotated",
    });
    expect(createMock).not.toHaveBeenCalled();
    expect(credentialRef).toBe(existing);
  });

  it("creates a new credential when rotate rejects a workspace mismatch", async () => {
    rotateMock.mockRejectedValue({
      code: "VALIDATION_ERROR",
      message: "validation error: credential reference does not belong to the workspace",
    });

    const credentialRef = await persistDatabaseConnectionPassword({
      input: postgresInput,
      secret: "secret",
      workspaceId: "ws-current",
    });

    expect(createMock).toHaveBeenCalledWith({
      workspaceId: "ws-current",
      kind: DATABASE_PASSWORD_KIND,
      label: "App DB",
      secret: "secret",
    });
    expect(credentialRef).toBe("unfour:ws-current:database-password:new-cred");
  });

  it("does not swallow non-mismatch rotate failures", async () => {
    rotateMock.mockRejectedValue(new Error("credential secret cannot be empty"));

    await expect(
      persistDatabaseConnectionPassword({
        input: postgresInput,
        secret: "secret",
        workspaceId: "ws-current",
      }),
    ).rejects.toThrow("credential secret cannot be empty");
    expect(createMock).not.toHaveBeenCalled();
  });

  it("keeps the existing reference when the password field is blank", async () => {
    const existing = "unfour:ws-current:database-password:cred-1";
    const credentialRef = await persistDatabaseConnectionPassword({
      input: { ...postgresInput, credentialRef: existing },
      secret: "  ",
      workspaceId: "ws-current",
    });

    expect(rotateMock).not.toHaveBeenCalled();
    expect(createMock).not.toHaveBeenCalled();
    expect(credentialRef).toBe(existing);
  });
});
