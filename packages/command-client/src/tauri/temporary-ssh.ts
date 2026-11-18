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
  terminalTaskBusy?: boolean;
  terminalRecoveryRequired?: boolean;
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
export type TemporarySshRoute = Pick<TemporarySshSummary, "handle" | "displayName" | "endpoint" | "terminalTaskBusy" | "terminalRecoveryRequired">;
export async function listTemporaryRoutes(workspaceId: string): Promise<TemporarySshRoute[]> {
  return (await listTemporarySsh(workspaceId))
    .filter((entry) => entry.workspaceId === workspaceId && entry.status === "connected")
    .map(({ handle, displayName, endpoint, terminalTaskBusy, terminalRecoveryRequired }) => ({ handle, displayName, endpoint,
      ...(terminalTaskBusy !== undefined && { terminalTaskBusy }), ...(terminalRecoveryRequired !== undefined && { terminalRecoveryRequired }) }));
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

export type TemporaryProbeKind = "exec" | "terminal" | "transport" | "curl" | "channels" | "sftp" | "tcp";
export type TemporaryProbeCheckStatus = "pass" | "fail" | "unsupported" | "inconclusive" | "cancelled";
export interface TemporaryProbeReport {
  runId: string;
  kind: TemporaryProbeKind;
  status: "running" | "completed" | "cancelled";
  startedAt: string;
  finishedAt: string | null;
  checks: {
    id: string;
    status: TemporaryProbeCheckStatus;
    code: string;
    expectedExitCode: number | null;
    actualExitCode: number | null;
  }[];
  probeFile: string | null;
  cleanup: "removed" | "not-created" | "unverified" | null;
}

/** Explicit, separately confirmed checks; listing/opening a terminal never probes. */
export const startTemporarySshProbe = (workspaceId: string, handle: string, kind: TemporaryProbeKind, confirmed: boolean) =>
  call<TemporaryProbeReport>("ssh_temporary_probe_start", { workspaceId, handle, kind, confirmed });
export const startTemporarySshTcpProbe = (workspaceId: string, handle: string, host: string, port: number, confirmed: boolean) =>
  call<TemporaryProbeReport>("ssh_temporary_probe_tcp_start", { workspaceId, handle, host, port, confirmed });
export const getTemporarySshProbe = (workspaceId: string, handle: string) =>
  call<TemporaryProbeReport | null>("ssh_temporary_probe_get", { workspaceId, handle });
export const cancelTemporarySshProbe = (workspaceId: string, handle: string, runId: string) =>
  call<void>("ssh_temporary_probe_cancel", { workspaceId, handle, runId });
