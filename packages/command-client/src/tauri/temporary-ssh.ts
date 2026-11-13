import { call, isTauriRuntime } from "./invoke";
import { listen } from "@tauri-apps/api/event";
import { forgetTemporaryPasswords, observeLiveRedisSources } from "./session-routes";
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
const latestLists = new Map<string, number>();
export async function listTemporarySsh(workspaceId: string) {
  if (!isTauriRuntime()) return [];
  const read = (latestLists.get(workspaceId) ?? 0) + 1;
  latestLists.set(workspaceId, read);
  const summaries = await call<TemporarySshSummary[]>("ssh_temporary_list", { workspaceId });
  if (latestLists.get(workspaceId) === read) {
    observeLiveRedisSources(workspaceId, new Set(summaries.filter((entry) => entry.workspaceId === workspaceId && entry.status === "connected").map((entry) => entry.handle)));
  }
  return summaries;
}
export async function listTemporaryRouteHandles(workspaceId: string): Promise<string[]> {
  const summaries = await listTemporarySsh(workspaceId);
  return summaries
    .filter((entry) => entry.workspaceId === workspaceId && entry.status === "connected")
    .map((entry) => entry.handle);
}
/** Use a short opaque identifier to distinguish sessions without exposing login data. */
export function temporaryRouteOptions(handles: string[], selected: string | null | undefined, label: (id: string) => string, unavailable: string) {
  const options = handles.map((handle) => ({ value: handle, label: label(handle.slice(-8)) }));
  if (selected && !handles.includes(selected)) options.push({ value: selected, label: unavailable });
  return options;
}
export const onTemporarySshLaunch = (notify: () => void) =>
  isTauriRuntime() ? listen("ssh://launch-pending", notify) : Promise.resolve(null);
export const claimTemporarySsh = (workspaceId: string, handle: string) => call<TemporarySshSummary>("ssh_temporary_claim", { workspaceId, handle });
export const startTemporarySsh = (workspaceId: string, handle: string) => call<void>("ssh_temporary_start", { workspaceId, handle });
export const trustTemporarySsh = (workspaceId: string, handle: string, fingerprint: string) => call<void>("ssh_temporary_trust", { workspaceId, handle, fingerprint });
export async function cancelTemporarySsh(workspaceId: string, handle: string) {
  await call<void>("ssh_temporary_cancel", { workspaceId, handle });
  latestLists.set(workspaceId, (latestLists.get(workspaceId) ?? 0) + 1);
  forgetTemporaryPasswords(workspaceId, handle);
}
export function closeTemporarySshWorkspace(workspaceId: string) {
  latestLists.set(workspaceId, (latestLists.get(workspaceId) ?? 0) + 1);
  forgetTemporaryPasswords(workspaceId);
  return isTauriRuntime() ? call<void>("ssh_temporary_close_workspace", { workspaceId }) : Promise.resolve();
}
export const readTemporarySshOutput = (workspaceId: string, handle: string) => call<SshSessionEvent[]>("ssh_temporary_output", { workspaceId, handle });
export const sendTemporarySshInput = (input: SshSessionInput) => call<SshSessionEvent>("ssh_temporary_input", { input });
export const resizeTemporarySsh = (input: SshResizeInput) => call<SshSessionEvent>("ssh_temporary_resize", { input });
