import type { SshSessionSummary } from "@unfour/command-client";
import { ConnectionStatus } from "@unfour/ui";
import {
  terminalSessionStatus,
  terminalSessionStatusLabel,
} from "../model/terminal-session-status";

export function TerminalSessionTabMeta({ session }: { session: SshSessionSummary }) {
  return (
    <ConnectionStatus
      label={terminalSessionStatusLabel(session)}
      status={terminalSessionStatus(session)}
    />
  );
}
