import { handleApiMock } from "./api";
import { handleFlowMock } from "./flow";
import { handleAppMock } from "./app";
import { handleDatabaseMock } from "./database";
import { handleRedisMock } from "./redis";
import { handleDiagnosticsMock } from "./diagnostics";
import { handleMcpMock } from "./mcp";
import { handleSecretStoreMock } from "./secret-store";
import { handleSftpMock } from "./sftp";
import { handleSshMock } from "./ssh";
import { handleSshTaskMock } from "./ssh-tasks";
import { handleTelemetryMock } from "./telemetry";
import { handleWorkspaceMock } from "./workspace";
import { UNHANDLED, type MockCommandHandler } from "./types";

const mockHandlers: MockCommandHandler[] = [
  handleFlowMock,
  handleDiagnosticsMock,
  handleWorkspaceMock,
  handleApiMock,
  handleAppMock,
  handleSecretStoreMock,
  handleDatabaseMock,
  handleRedisMock,
  handleSshMock,
  handleSshTaskMock,
  handleSftpMock,
  handleTelemetryMock,
  handleMcpMock,
];

export async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (args?.temporaryHandle != null) throw { code: "SSH_TUNNEL_TEMPORARY_UNAVAILABLE" };
  for (const handler of mockHandlers) {
    const value = handler<T>(command, args);
    const result = value instanceof Promise ? await value : value;
    if (result !== UNHANDLED) {
      // Mirror IPC value semantics before mutable mock records reach callers.
      return structuredClone(result);
    }
  }

  throw new Error(`Mock command is not implemented: ${command}`);
}
