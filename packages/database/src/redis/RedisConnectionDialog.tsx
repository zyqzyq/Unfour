import { useEffect, useState, type ReactNode } from "react";
import { listSshConnections, saveRedisConnection, testRedisConnectionInput, type RedisConnection, type RedisConnectionInput } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle, Input, SecretInput, Select, useI18n } from "@unfour/ui";
import { redisErrorMessage } from "./redis-error";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="block space-y-1 text-xs text-[var(--u-color-text-muted)]"><span>{label}</span>{children}</label>;
}

type JumpOption = { value: string; label: string; revision: number };

function useJumpHosts(workspaceId: string) {
  const [result, setResult] = useState<{ workspaceId: string; state: "ready" | "failed"; options: JumpOption[] }>();
  useEffect(() => {
    let current = true;
    listSshConnections(workspaceId).then((records) => {
      if (!current) return;
      const options = records.filter((r) => r.workspaceId === workspaceId && !r.deletedAt)
        .map((r) => ({ value: r.id, label: `${r.name} · ${r.username}@${r.host}:${r.port}`, revision: r.revision }));
      setResult({ workspaceId, state: "ready", options });
    }).catch(() => { if (current) setResult({ workspaceId, state: "failed", options: [] }); });
    return () => { current = false; };
  }, [workspaceId]);
  return result?.workspaceId === workspaceId ? result : { state: "loading", options: [] };
}

export function RedisConnectionDialog({ workspaceId, original, latest, initialName, onClose, onSaved }: {
  workspaceId: string;
  original: RedisConnection | null;
  latest: RedisConnection | undefined;
  initialName: string;
  onClose: () => void;
  onSaved: (connection: RedisConnection) => void;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState<RedisConnectionInput>(() => ({
    workspaceId, id: original?.id, expectedRevision: original?.revision,
    name: original?.name ?? initialName, host: original?.host ?? "127.0.0.1", port: original?.port ?? 6379,
    dbIndex: original?.dbIndex ?? 0, username: original?.username ?? null, tls: original?.tls ?? false,
    allowWrites: original?.allowWrites ?? false,
    sshConnectionId: original?.sshConnectionId ?? null,
  }));
  const [password, setPassword] = useState("");
  const [clear, setClear] = useState(false);
  const [pending, setPending] = useState<"save" | "test" | null>(null);
  const [tested, setTested] = useState(false);
  const [error, setError] = useState("");
  const { options: jumps, state: listState } = useJumpHosts(workspaceId);
  const unknownJump = Boolean(form.sshConnectionId && !jumps.some((item) => item.value === form.sshConnectionId));
  const replacePassword = Boolean(original?.hasPassword && (
    (original.sshConnectionId ?? null) !== form.sshConnectionId || original.routeNeedsReview
    || form.host !== original.host || form.port !== original.port || form.username !== original.username || form.tls !== original.tls
  ));
  const changed = Boolean(original && latest?.revision !== original.revision);
  const valid = !unknownJump && (!replacePassword || password || clear) && form.name.trim() && form.host.trim() && Number.isInteger(form.port) && form.port > 0 && form.port <= 65535 && Number.isSafeInteger(form.dbIndex) && form.dbIndex >= 0;

  function edit(patch: Partial<RedisConnectionInput>) {
    const changesTarget = ["host", "port", "username", "tls", "dbIndex", "sshConnectionId"].some((key) => key in patch);
    setForm((current) => ({ ...current, ...patch, ...(changesTarget && { allowWrites: false }) }));
    setTested(false); setError("");
  }
  async function submit(action: "save" | "test") {
    if (!valid || pending || changed) return;
    setError(""); setTested(false); setPending(action);
    const secret = clear ? "" : password || null;
    const input = { ...form, expectedSshRevision: jumps.find(j => j.value === form.sshConnectionId)?.revision };
    try {
      if (action === "test") { await testRedisConnectionInput(input, secret); setTested(true); }
      else onSaved(await saveRedisConnection(input, secret));
    } catch (cause) {
      setError(redisErrorMessage(cause, t));
    } finally {
      setPending(null);
    }
  }
  return <Dialog open onOpenChange={(open) => { if (!open && !pending) onClose(); }}>
    <DialogContent title={t("redis.settings")}>
      <DialogHeader><DialogTitle>{t("redis.settings")}</DialogTitle></DialogHeader>
      <form className="flex min-h-0 flex-col" onSubmit={(event) => { event.preventDefault(); void submit("save"); }}>
        <fieldset className="contents" disabled={Boolean(pending)}>
          <DialogBody className="space-y-3">
            <Field label={t("database.fields.name")}><Input required value={form.name} onChange={(event) => edit({ name: event.target.value })} /></Field>
            <div className="grid grid-cols-[minmax(0,1fr)_96px] gap-3">
              <Field label={t("database.fields.host")}><Input required value={form.host} onChange={(event) => edit({ host: event.target.value })} /></Field>
              <Field label={t("database.fields.port")}><Input type="number" required min={1} max={65535} value={form.port || ""} onChange={(event) => edit({ port: Number(event.target.value) })} /></Field>
            </div>
            <Field label={t("redis.logicalDb")}><Input required type="number" min={0} value={form.dbIndex} onChange={(event) => edit({ dbIndex: Number(event.target.value) })} /></Field>
            <Field label={t("redis.route.label")}><Select value={form.sshConnectionId ?? ""}
              options={[{ value: "", label: t("redis.route.direct") }, ...jumps,
                ...(unknownJump ? [{ value: form.sshConnectionId!, label: listState === "loading" ? t("redis.route.loading") : t("redis.route.missing") }] : [])]}
              onChange={(event) => edit({ sshConnectionId: event.target.value || null })} /></Field>
            {listState === "ready" && !jumps.length && <p className="text-xs text-[var(--u-color-text-muted)]">{t("redis.route.noConnections")}</p>}
            {listState === "failed" && <p role="alert" className="text-xs">{t("redis.route.listFailed")}</p>}
            {original?.routeNeedsReview && <p role="status" className="text-xs">{t("network.errors.SSH_TUNNEL_ROUTE_CHANGED")}</p>}
            <Field label={t("database.fields.username")}><Input value={form.username ?? ""} onChange={(event) => edit({ username: event.target.value || null })} /></Field>
            <Field label={t("database.fields.password")}><SecretInput value={password} placeholder={original?.hasPassword && !clear && !replacePassword ? t("database.fields.passwordKeep") : undefined} onChange={(event) => { setPassword(event.target.value); setClear(false); setTested(false); }} /></Field>
            {replacePassword && !password && !clear && <p className="text-xs text-[var(--u-color-text-muted)]">{t("redis.route.replacePassword")}</p>}
            {original?.hasPassword && <label className="flex items-center gap-2 text-xs"><input type="checkbox" checked={clear} onChange={(event) => { setClear(event.target.checked); setPassword(""); setTested(false); }} />{t("common.secret.clear")}</label>}
            <label className="flex items-center gap-2 text-xs"><input className="accent-[var(--u-color-primary)]" type="checkbox" checked={form.tls} onChange={(event) => edit({ tls: event.target.checked })} />{t("redis.tls")}</label>
            <label className="flex items-center gap-2 text-xs"><input className="accent-[var(--u-color-primary)]" type="checkbox" checked={form.allowWrites ?? false} onChange={(event) => edit({ allowWrites: event.target.checked })} />{t("redis.write.allow")}</label>
            {changed && <p role="alert" className="text-xs">{t("common.externalChange")}</p>}
            {error && <p role="alert" className="select-text text-xs text-[var(--u-color-danger)]">{error}</p>}
            {tested && <p role="status" className="text-xs text-[var(--u-color-success)]">{t("database.connection.testSuccess")}</p>}
          </DialogBody>
          <DialogFooter>
            <Button className="mr-auto" type="button" variant="outline" size="sm" disabled={!valid || changed} onClick={() => void submit("test")}>{t(pending === "test" ? "database.connection.testing" : "database.connection.test")}</Button>
            <Button type="button" variant="ghost" size="sm" onClick={onClose}>{t("common.confirm.cancel")}</Button>
            <Button type="submit" size="sm" disabled={!valid || changed}>{t("common.actions.save")}</Button>
          </DialogFooter>
        </fieldset>
      </form>
    </DialogContent>
  </Dialog>;
}
