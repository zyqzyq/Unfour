import {
  createCredential,
  rotateCredential,
  type DatabaseConnectionInput,
} from "@unfour/command-client";

export const DATABASE_PASSWORD_KIND = "database-password";

export function emptyDatabaseConnectionForm(
  workspaceId: string,
): DatabaseConnectionInput {
  return { workspaceId, name: "", driver: "sqlite", sqlitePath: "" };
}

export function isCredentialWorkspaceMismatch(error: unknown): boolean {
  return credentialErrorMessage(error).includes(
    "credential reference does not belong to the workspace",
  );
}

export async function persistDatabaseConnectionPassword({
  input,
  secret,
  workspaceId,
}: {
  input: DatabaseConnectionInput;
  secret: string;
  workspaceId: string;
}): Promise<string | null> {
  const credentialRef = input.credentialRef?.trim() || null;
  if (input.driver === "sqlite" || !secret.trim()) {
    return credentialRef;
  }

  if (credentialRef) {
    try {
      await rotateCredential({ workspaceId, credentialRef, secret });
      return credentialRef;
    } catch (error) {
      if (!isCredentialWorkspaceMismatch(error)) {
        throw error;
      }
    }
  }

  const metadata = await createCredential({
    workspaceId,
    kind: DATABASE_PASSWORD_KIND,
    label: input.name,
    secret,
  });
  return metadata.credentialRef;
}

function credentialErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message: unknown }).message;
    return typeof message === "string" ? message : "";
  }
  return "";
}
