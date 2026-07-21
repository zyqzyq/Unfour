import { handleApiMock } from "./api";
import { handleAppMock } from "./app";
import { handleDatabaseMock } from "./database";
import { handleDiagnosticsMock } from "./diagnostics";
import { handleMcpMock } from "./mcp";
import { handleSecretStoreMock } from "./secret-store";
import { handleSftpMock } from "./sftp";
import { handleSshMock } from "./ssh";
import { handleWorkspaceMock } from "./workspace";
import { UNHANDLED, type MockCommandHandler } from "./types";

const mockHandlers: MockCommandHandler[] = [
  handleDiagnosticsMock,
  handleWorkspaceMock,
  handleApiMock,
  handleAppMock,
  handleSecretStoreMock,
  handleDatabaseMock,
  handleSshMock,
  handleSftpMock,
  handleMcpMock,
];

export async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  for (const handler of mockHandlers) {
    const result = await handler<T>(command, args);
    if (result !== UNHANDLED) {
      return result;
    }
  }

  throw new Error(`Mock command is not implemented: ${command}`);
}
