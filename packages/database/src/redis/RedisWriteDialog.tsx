import { useRef, useState } from "react";
import { mutateRedisKey, type RedisConnection } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle, Input, Select, useI18n } from "@unfour/ui";
import { redisErrorMessage } from "./redis-error";
import { hexBytes, readableText, redisName } from "./redis-bytes";
import { createKind, initialWriteForm, keyCaption, parseWriteValue, prepareWrite, writesValue, type PreparedWrite, type WriteForm, type WriteTarget } from "./redis-write";

const commands = {
  createString: "SET · NX", updateString: "SET · XX · KEEPTTL", createHash: "HSETNX",
  addHashField: "HSETNX", updateHashField: "HSET / HSETEX FXX KEEPTTL", deleteHashField: "HDEL", setTtl: "PEXPIRE", persist: "PERSIST", deleteKey: "UNLINK",
};

export function RedisWriteDialog({ connection, target, onClose, onResolve, onInvalidate, onRefreshConnections }: {
  connection: RedisConnection; target: WriteTarget; onClose: () => void;
  onResolve: (write: PreparedWrite, outcome: "applied" | "reread") => void;
  onInvalidate?: () => void;
  onRefreshConnections: () => Promise<unknown>;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState(() => initialWriteForm(target));
  const immediateReview = target.kind === "deleteKey" || target.kind === "deleteHashField" || target.kind === "persist";
  const [review, setReview] = useState(immediateReview);
  const [submitted, setSubmitted] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [recovery, setRecovery] = useState<"reread" | "connectionChanged" | null>(null);
  const sending = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const { ready, issue } = prepareWrite(target, form);
  const { kind } = form;
  const creating = createKind(kind);
  const destructive = kind === "deleteKey" || kind === "deleteHashField";
  const title = t(`redis.write.actions.${kind}`);
  const sample = parseWriteValue(form.value, form.hex);
  const alternateValue = sample === null ? null : form.hex ? readableText(sample) : hexBytes(sample);
  const encodingChange = alternateValue !== null && !(form.hex && alternateValue.includes("\r")) ? alternateValue : null;
  const switchEncoding = () => {
    if (encodingChange !== null) setForm({ ...form, hex: !form.hex, value: encodingChange });
  };
  const close = () => {
    if (sending.current) return;
    if (recovery === "reread" && ready) onResolve(ready, recovery);
    else onClose();
  };
  async function refreshConnections() {
    if (sending.current) return;
    sending.current = true;
    setPending(true);
    try {
      await onRefreshConnections();
      onClose();
    } catch {
      setError(t("redis.listFailed"));
    } finally {
      sending.current = false;
      setPending(false);
    }
  }
  const fieldName = kind === "createHash" || kind === "addHashField";
  const ttlField = kind === "createString" || kind === "setTtl";
  const showReview = () => {
    setSubmitted(true);
    if (ready) { setReview(true); cancelButton.current?.focus(); }
  };
  async function apply() {
    if (!ready || !connection.allowWrites || !review || sending.current || recovery) return;
    sending.current = true;
    setPending(true); setError("");
    try {
      await mutateRedisKey({
        workspaceId: connection.workspaceId, connectionId: connection.id, expectedRevision: connection.revision,
        keyBase64: ready.keyBase64, mutation: ready.mutation, confirmed: true,
      });
      onResolve(ready, "applied");
    } catch (cause) {
      const code = cause && typeof cause === "object" && "code" in cause ? String(cause.code) : "";
      setError(redisErrorMessage(cause, t));
      const next = code === "ROW_CONFLICT" || code === "NOT_FOUND" ? "connectionChanged"
        : ["REDIS_WRITE_CONFLICT", "REDIS_WRITE_OUTCOME_UNKNOWN", "REDIS_KEY_CHANGED"].includes(code) ? "reread" : null;
      setRecovery(next);
      if (next) onInvalidate?.();
    } finally {
      sending.current = false; setPending(false);
    }
  }
  return <Dialog open onOpenChange={(open) => { if (!open) close(); }}>
    <DialogContent title={title} onOpenAutoFocus={(event) => {
      if (immediateReview) { event.preventDefault(); cancelButton.current?.focus(); }
    }}>
      <DialogHeader><DialogTitle>{title}</DialogTitle></DialogHeader>
      <DialogBody className="space-y-3">
        <p className="select-text break-all text-xs text-[var(--u-color-text-muted)]">{connection.name} · {connection.host}:{connection.port} · DB {connection.dbIndex}</p>
        {review && ready
          ? <WriteReview ready={ready} form={form} target={target} />
          : <fieldset disabled={pending} className="grid gap-3 text-xs">
            {creating
              ? <>
                <label>{t("redis.type")}<Select value={kind} options={[{ value: "createString", label: "String" }, { value: "createHash", label: "Hash" }]}
                  onChange={(event) => setForm({ ...form, kind: event.target.value === "createHash" ? "createHash" : "createString" })} /></label>
                <label>{t("redis.key")}<Input value={form.key} onChange={(event) => setForm({ ...form, key: event.target.value })} /></label>
              </>
              : <strong className="select-text break-all font-mono">{redisName(target.keyBase64)}</strong>}
            {fieldName && <label>{t("redis.hashField")}<Input value={form.field} onChange={(event) => setForm({ ...form, field: event.target.value })} /></label>}
            {target.fieldBase64 !== undefined && !fieldName && <p className="select-text break-all">{t("redis.hashField")} · {redisName(target.fieldBase64)}</p>}
            {writesValue(kind) && <label className="grid gap-1">{form.hex ? t("redis.write.hexValue") : t("redis.write.newValue")}
              <textarea className="min-h-24 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono"
                value={form.value} onChange={(event) => setForm({ ...form, value: event.target.value })} />
            </label>}
            {writesValue(kind) && <Button className="justify-self-start" type="button" size="sm" variant="ghost"
              disabled={encodingChange === null} onClick={switchEncoding}>
              {t(form.hex ? "redis.write.useText" : "redis.write.useHex")}
            </Button>}
            {ttlField && <label>{t(kind === "setTtl" ? "redis.write.seconds" : "redis.write.optionalSeconds")}
              <Input inputMode="decimal" value={form.seconds} placeholder={kind === "createString" ? t("redis.persistent") : "60"}
                onChange={(event) => setForm({ ...form, seconds: event.target.value })} />
            </label>}
            {kind === "setTtl" && target.info && <p>{t("redis.write.observedExpiry")} · {target.info.pttlMs === -1 ? t("redis.persistent") : t("redis.milliseconds", { count: target.info.pttlMs })}</p>}
          </fieldset>}
        {(error || (submitted && issue)) && <p role="alert" className="text-xs text-[var(--u-color-danger)]">{error || t(issue!)}</p>}
      </DialogBody>
      <DialogFooter>
        <Button ref={cancelButton} type="button" variant="ghost" disabled={pending} onClick={close}>{t("common.confirm.cancel")}</Button>
        {review && !immediateReview && !recovery && <Button variant="outline" disabled={pending} onClick={() => { setReview(false); setError(""); }}>{t("redis.write.back")}</Button>}
        {recovery && ready
          ? <Button disabled={pending} onClick={recovery === "connectionChanged" ? () => void refreshConnections() : close}>{t(recovery === "connectionChanged" ? "database.actions.refreshConnections" : "redis.write.inspectAgain")}</Button>
          : review ? <Button type="button" variant={destructive ? "danger" : "default"} disabled={!ready || pending} onClick={() => void apply()}>
            {t(pending ? "redis.write.applying" : "redis.write.apply")}
          </Button> : <Button onClick={showReview}>{t("redis.write.review")}</Button>}
      </DialogFooter>
    </DialogContent>
  </Dialog>;
}

function WriteReview({ ready, form, target }: { ready: PreparedWrite; form: WriteForm; target: WriteTarget }) {
  const { t } = useI18n();
  const { kind } = form;
  const { mutation } = ready;
  return <div className="grid gap-2 text-xs">
    <strong className="select-text break-all font-mono">{redisName(ready.keyBase64)}</strong>
    {target.info?.exists && <p>{t("redis.type")} · {target.info.keyType}</p>}
    <span className="font-mono">{commands[kind]}{kind === "createString" && "ttlMs" in mutation && " · PX"}</span>
    {"fieldBase64" in mutation && <p className="select-text break-all">{t("redis.hashField")} · {redisName(mutation.fieldBase64)}</p>}
    {"expectedBase64" in mutation && <div>{t("redis.write.observedValue")}
      <pre className="mt-1 max-h-24 select-text overflow-auto whitespace-pre-wrap break-all font-mono">{keyCaption(mutation.expectedBase64, t("redis.emptyValue"))}</pre>
    </div>}
    {"valueBase64" in mutation && <div>{t(form.hex ? "redis.write.hexValue" : "redis.write.newValue")}
      <pre className="mt-1 max-h-32 select-text overflow-auto whitespace-pre-wrap break-all font-mono">{form.value || t("redis.emptyValue")}</pre>
    </div>}
    {"ttlMs" in mutation && <p>TTL · {t("redis.milliseconds", { count: mutation.ttlMs ?? 0 })}</p>}
    {kind === "setTtl" && <p>{t("redis.write.expiryFromApply")}</p>}
    {(kind === "updateString" || kind === "updateHashField") && <p>{t("redis.write.keepExpiry")}</p>}
    {createKind(kind) && <p>{t("redis.write.createOnly")}</p>}
    {kind === "deleteHashField" && <p className="text-[var(--u-color-warning)]">{t("redis.write.deleteFieldWarning")}</p>}
    {kind === "deleteKey" && <p className="text-[var(--u-color-warning)]">{t("redis.write.deleteWarning")}</p>}
    {kind === "persist" && <p>{t("redis.write.persistHelp")}</p>}
  </div>;
}
