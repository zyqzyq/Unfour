import { useState, type ReactNode } from "react";
import { saveRedisConnection, testRedisConnectionInput, type RedisConnection, type RedisConnectionInput } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle, Input, SecretInput, useI18n } from "@unfour/ui";
import { redisErrorMessage } from "./redis-error";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="block space-y-1 text-xs text-[var(--u-color-text-muted)]"><span>{label}</span>{children}</label>;
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
  }));
  const [password, setPassword] = useState("");
  const [clear, setClear] = useState(false);
  const [pending, setPending] = useState<"save" | "test" | null>(null);
  const [tested, setTested] = useState(false);
  const [error, setError] = useState("");
  const changed = Boolean(original && latest?.revision !== original.revision);
  const valid = form.name.trim() && form.host.trim() && Number.isInteger(form.port) && form.port > 0 && form.port <= 65535 && Number.isSafeInteger(form.dbIndex) && form.dbIndex >= 0;

  function edit(patch: Partial<RedisConnectionInput>) {
    const changesTarget = ["host", "port", "username", "tls", "dbIndex"].some((key) => key in patch);
    setForm((current) => ({ ...current, ...patch, ...(changesTarget && { allowWrites: false }) }));
    setTested(false); setError("");
  }
  async function submit(action: "save" | "test") {
    if (!valid || pending || changed) return;
    setError(""); setTested(false); setPending(action);
    const secret = clear ? "" : password || null;
    try {
      if (action === "test") { await testRedisConnectionInput(form, secret); setTested(true); }
      else onSaved(await saveRedisConnection(form, secret));
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
            <Field label={t("database.fields.username")}><Input value={form.username ?? ""} onChange={(event) => edit({ username: event.target.value || null })} /></Field>
            <Field label={t("database.fields.password")}><SecretInput value={password} placeholder={original?.hasPassword && !clear ? t("database.fields.passwordKeep") : undefined} onChange={(event) => { setPassword(event.target.value); setClear(false); setTested(false); }} /></Field>
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
