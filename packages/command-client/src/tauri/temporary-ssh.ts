import { call, isTauriRuntime } from "./invoke";
import type { SshSessionEvent, SshSessionInput, SshResizeInput } from "../types";

export interface TemporarySshSummary {
  handle: string;
  workspaceId: string | null;
  status: "pending" | "claimed" | "connecting" | "awaiting-trust" | "connected" | "disconnected" | "expired";
  expiresInSeconds: number;
  fingerprint: string | null;
  errorCode: string | null;
}
// Browser previews have no injection mechanism or fake credential parser.
export const listTemporarySsh = (workspaceId: string) => isTauriRuntime()
  ? call<TemporarySshSummary[]>("ssh_temporary_list", { workspaceId }) : Promise.resolve([]);
export const claimTemporarySsh = (workspaceId: string, handle: string) => call<TemporarySshSummary>("ssh_temporary_claim", { workspaceId, handle });
export const startTemporarySsh = (workspaceId: string, handle: string) => call<void>("ssh_temporary_start", { workspaceId, handle });
export const trustTemporarySsh = (workspaceId: string, handle: string, fingerprint: string) => call<void>("ssh_temporary_trust", { workspaceId, handle, fingerprint });
export const cancelTemporarySsh = (workspaceId: string, handle: string) => call<void>("ssh_temporary_cancel", { workspaceId, handle });
export const closeTemporarySshWorkspace = (workspaceId: string) => isTauriRuntime()
  ? call<void>("ssh_temporary_close_workspace", { workspaceId }) : Promise.resolve();
export const readTemporarySshOutput = (workspaceId: string, handle: string) => call<SshSessionEvent[]>("ssh_temporary_output", { workspaceId, handle });
export const sendTemporarySshInput = (input: SshSessionInput) => call<SshSessionEvent>("ssh_temporary_input", { input });
export const resizeTemporarySsh = (input: SshResizeInput) => call<SshSessionEvent>("ssh_temporary_resize", { input });
