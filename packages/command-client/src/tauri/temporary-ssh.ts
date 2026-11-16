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
  displayName: string;
  endpoint: string;
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
  return (await listTemporaryRoutes(workspaceId)).map((entry) => entry.handle);
}
export type TemporarySshRoute = Pick<TemporarySshSummary, "handle" | "displayName" | "endpoint">;
export async function listTemporaryRoutes(workspaceId: string): Promise<TemporarySshRoute[]> {
  return (await listTemporarySsh(workspaceId))
    .filter((entry) => entry.workspaceId === workspaceId && entry.status === "connected")
    .map(({ handle, displayName, endpoint }) => ({ handle, displayName, endpoint }));
}
/** Only allowlisted display metadata; authentication fields never reach the UI. */
export function temporarySshLabel(entry: TemporarySshRoute, fallback: string) {
  const name = entry.displayName?.trim();
  const endpoint = entry.endpoint?.trim();
  return name && endpoint && name !== endpoint ? `${name} · ${endpoint}` : name || endpoint || fallback;
}
export function temporaryRouteOptions(routes: (TemporarySshRoute | string)[], selected: string | null | undefined, label: (id: string) => string, unavailable: string) {
  const options = routes.map((route) => {
    const handle = typeof route === "string" ? route : route.handle;
    const fallback = label(handle.slice(-8));
    return { value: handle, label: typeof route === "string" ? fallback : temporarySshLabel(route, fallback) };
  });
  if (selected && !options.some((option) => option.value === selected)) options.push({ value: selected, label: unavailable });
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
