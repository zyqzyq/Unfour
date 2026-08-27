import { useEffect, useState } from "react";
import { Button, ErrorState, LoadingState, useI18n } from "@unfour/ui";
import {
  keepLocalConflict,
  listSyncConflicts,
  syncErrorCode,
  useRemoteConflict,
} from "./syncApi";
import type { SyncConflict } from "./syncTypes";
import { syncErrorMessageKey } from "./syncViewModel";

function payloadLabel(conflict: SyncConflict): string {
  for (const payload of [conflict.localPayload, conflict.remotePayload]) {
    const value = payload?.name ?? payload?.key;
    if (typeof value === "string" && value.trim()) return value;
  }
  return conflict.entityId;
}

function payloadValue(payload: Record<string, unknown> | null): string {
  if (!payload) return "—";
  for (const key of ["value", "name"]) {
    const value = payload[key];
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value);
  }
  return "—";
}

function conflictTitleKey(type: SyncConflict["entityType"]): string {
  if (type === "workspaceVariable" || type === "workspaceEnvironmentVariable") return "cloudSync.conflict.variableTitle";
  if (type === "workspaceEnvironment") return "cloudSync.conflict.environmentTitle";
  if (type === "connection") return "cloudSync.conflict.connectionTitle";
  if (type === "apiCollection" || type === "apiFolder" || type === "apiRequest") return "cloudSync.conflict.apiTitle";
  if (type === "sshTask" || type === "sshTaskStep") return "cloudSync.conflict.sshTaskTitle";
  return "cloudSync.conflict.workspaceTitle";
}

export function SyncConflictList({ onResolved, workspaceId }: { onResolved(): void; workspaceId: string }) {
  const { t } = useI18n();
  const [items, setItems] = useState<SyncConflict[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setErrorCode(null);
    try {
      setItems(await listSyncConflicts(workspaceId));
    } catch (error) {
      setErrorCode(syncErrorCode(error));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { void load(); }, [workspaceId]);

  const resolve = async (conflict: SyncConflict, choice: "local" | "remote") => {
    const key = `${conflict.entityType}:${conflict.entityId}`;
    setBusyKey(key);
    setErrorCode(null);
    try {
      const request = choice === "local" ? keepLocalConflict : useRemoteConflict;
      await request(workspaceId, conflict.entityType, conflict.entityId);
      await load();
      onResolved();
    } catch (error) {
      setErrorCode(syncErrorCode(error));
    } finally {
      setBusyKey(null);
    }
  };

  if (loading) return <LoadingState />;
  return <div className="flex flex-col gap-3">
    {errorCode && <ErrorState>{t(syncErrorMessageKey(errorCode))}</ErrorState>}
    {items.map((conflict) => {
      const key = `${conflict.entityType}:${conflict.entityId}`;
      const label = payloadLabel(conflict);
      const deleted = conflict.operation === "delete";
      return <section className="rounded-[var(--u-radius-md)] border border-[var(--u-color-warning)] p-3" key={key}>
        <h4 className="text-sm font-semibold">{t(conflictTitleKey(conflict.entityType))}</h4>
        <p className="mt-1 font-medium">{label}</p>
        {deleted ? <p className="mt-2 text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.conflict.deletedDescription", { name: label })}</p> : <div className="mt-2 grid grid-cols-2 gap-2">
          <div><p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.conflict.thisDevice")}</p><p className="mt-1 break-words rounded bg-[var(--u-color-surface-muted)] p-2 text-xs">{payloadValue(conflict.localPayload)}</p></div>
          <div><p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.conflict.cloud")}</p><p className="mt-1 break-words rounded bg-[var(--u-color-surface-muted)] p-2 text-xs">{payloadValue(conflict.remotePayload)}</p></div>
        </div>}
        {conflict.localSecretPresent !== null && <p className="mt-2 text-xs text-[var(--u-color-text-muted)]">{conflict.localSecretPresent ? t("cloudSync.localSecretPresent") : t("cloudSync.localSecretMissing")}</p>}
        <div className="mt-3 flex flex-wrap gap-2">
          <Button disabled={Boolean(busyKey)} onClick={() => void resolve(conflict, "local")} size="sm" type="button">{deleted ? t("cloudSync.conflict.keepLocalVariable") : t("cloudSync.conflict.useThisDevice")}</Button>
          <Button disabled={Boolean(busyKey)} onClick={() => void resolve(conflict, "remote")} size="sm" type="button" variant="outline">{deleted ? t("cloudSync.conflict.acceptCloudDeletion") : t("cloudSync.conflict.useCloud")}</Button>
        </div>
        <details className="mt-3 text-xs"><summary className="cursor-pointer text-[var(--u-color-text-muted)]">{t("cloudSync.technicalDetails")}</summary><pre className="mt-2 max-h-48 overflow-auto rounded bg-[var(--u-color-surface-muted)] p-2 text-[11px]">{JSON.stringify(conflict, null, 2)}</pre></details>
      </section>;
    })}
  </div>;
}
