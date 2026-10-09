import { inspectMockCredential } from "./helpers";
import { mockStore } from "./state";
import { UNHANDLED, type MockResult } from "./types";
import type {
  CredentialCreateInput,
  CredentialDeleteInput,
  CredentialInspectInput,
  CredentialMetadata,
  CredentialRotateInput,
} from "../../types";

export function handleSecretStoreMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "connection_secret_reveal") {
    const rows = args?.connectionType === "ssh" ? mockStore.sshConnections : args?.connectionType === "database" ? mockStore.databaseConnections : [];
    const connection = rows.find((row) => row.workspaceId === args?.workspaceId && row.id === args?.connectionId && row.credentialRef === args?.credentialRef);
    if (!connection?.credentialRef || !(connection.credentialRef in mockStore.credentials)) throw new Error("CONNECTION_CREDENTIAL_UNAVAILABLE");
    inspectMockCredential(connection.workspaceId, connection.credentialRef);
    return mockStore.credentials[connection.credentialRef] as T;
  }
  if (command === "credential_create") {
    const input = args?.input as CredentialCreateInput;
    const credentialRef = `unfour:${input.workspaceId}:${input.kind}:${crypto.randomUUID()}`;
    mockStore.credentials[credentialRef] = input.secret;
    return ({
      workspaceId: input.workspaceId,
      kind: input.kind,
      label: input.label,
      credentialRef,
    } satisfies CredentialMetadata) as T;
  }

  if (command === "credential_delete") {
    const input = args?.input as CredentialDeleteInput;
    delete mockStore.credentials[input.credentialRef];
    return undefined as T;
  }

  if (command === "credential_inspect") {
    const input = args?.input as CredentialInspectInput;
    const metadata = inspectMockCredential(input.workspaceId, input.credentialRef);
    return metadata as T;
  }

  if (command === "credential_rotate") {
    const input = args?.input as CredentialRotateInput;
    const metadata = inspectMockCredential(input.workspaceId, input.credentialRef);
    mockStore.credentials[input.credentialRef] = input.secret;
    return {
      ...metadata,
      label: "Rotated credential",
    } as T;
  }

  return UNHANDLED;
}
