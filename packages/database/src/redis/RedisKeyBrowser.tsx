import { useEffect, useRef, useState } from "react";
import { inspectRedisKey, scanRedisKeys, type RedisConnection, type RedisFlowIntent, type RedisKeyInfo } from "@unfour/command-client";
import { Button, Input, useI18n } from "@unfour/ui";
import { RedisKeyDetails } from "./RedisKeyDetails";
import { isRedisCollectionKind, readableText, redisName, textKey } from "./redis-bytes";
import { redisErrorMessage } from "./redis-error";
import { redisRouteLabel } from "./redis-route";
import { RedisCollectionBrowser } from "./RedisCollectionBrowser";
import { RedisKeyWriteMenu } from "./RedisKeyWriteMenu";
import { RedisWriteDialog } from "./RedisWriteDialog";
import { keyCaption, type PreparedWrite, type WriteTarget } from "./redis-write";

type KeyResult = { encoded: string; label: string; info: RedisKeyInfo; readAt: Date; fieldBase64?: string };
type BrowserProps = { connection: RedisConnection; active?: boolean; onCreateFlow?: (intent: RedisFlowIntent) => void;
  onRefreshConnections?: () => Promise<unknown> };
type Scan = { cursor: string; keys: string[]; limited: boolean };
const MAX_VISIBLE_KEYS = 1000;
const MAX_ENCODED_KEY_CHARACTERS = 700_000;

function mergeScanBatch(previous: Scan | undefined, batch: { keys: string[]; cursor: string }): Scan {
  const keys = new Set(previous?.keys);
  let size = Array.from(keys).reduce((sum, key) => sum + key.length, 0);
  for (const key of batch.keys) {
    if (keys.has(key)) continue;
    size += key.length;
    if (keys.size >= MAX_VISIBLE_KEYS || size > MAX_ENCODED_KEY_CHARACTERS) {
      return { cursor: batch.cursor, keys: [...keys], limited: true };
    }
    keys.add(key);
  }
  return { cursor: batch.cursor, keys: [...keys], limited: false };
}

export function RedisKeyBrowser(props: BrowserProps) {
  const { connection } = props;
  return <RedisKeySession key={`${connection.workspaceId}:${connection.id}:${connection.revision}`} {...props} />;
}

function RedisKeySession({ connection, active = true, onCreateFlow, onRefreshConnections }: BrowserProps) {
  const { t } = useI18n();
  const [mode, setMode] = useState<"exact" | "pattern">("exact");
  const [key, setKey] = useState("");
  const [field, setField] = useState("");
  const [pattern, setPattern] = useState("*");
  const [result, setResult] = useState<KeyResult | null>(null);
  const [scan, setScan] = useState<Scan | null>(null);
  const [task, setTask] = useState<"inspect" | "scan" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [write, setWrite] = useState<WriteTarget | null>(null);
  const [notice, setNotice] = useState("");
  const generation = useRef(0);
  const live = useRef(true);
  const flowKey = result ? readableText(result.encoded) : null;
  useEffect(() => {
    live.current = true;
    return () => { live.current = false; };
  }, []);
  if (!active && write) setWrite(null);

  function cancel() {
    generation.current++;
    setTask(null);
    setError(null);
    setNotice("");
  }

  function showMode(next: typeof mode) {
    if (next === mode) return;
    cancel();
    setMode(next);
    setResult(null);
    setField("");
  }

  async function runInspect(encoded: string, label: string, hashField?: string, applied = false) {
    const request = ++generation.current;
    setTask("inspect");
    setError(null);
    setNotice(applied ? t("redis.write.applied") : "");
    if (hashField === undefined) setResult(null);
    else setResult((previous) => previous && { ...previous, fieldBase64: hashField, info: { ...previous.info, fieldFound: null, preview: null } });
    try {
      const info = await inspectRedisKey(connection.workspaceId, connection.id, encoded, hashField, connection.revision);
      if (live.current && request === generation.current) setResult({ encoded, label, info, readAt: new Date(), fieldBase64: hashField });
    } catch (cause) {
      if (live.current && request === generation.current) setError(applied
        ? t("redis.write.refreshFailed", { error: redisErrorMessage(cause, t) }) : redisErrorMessage(cause, t));
    } finally {
      if (live.current && request === generation.current) setTask(null);
    }
  }

  function refreshWrittenKey(prepared: PreparedWrite, outcome: "applied" | "reread") {
    if (!live.current) return;
    const { keyBase64, mutation } = prepared;
    setWrite(null); setScan(null); setResult(null);
    const text = readableText(keyBase64);
    // A single-line input strips CR/LF. Keep these keys in the pattern browser.
    if (text !== null && !/[\r\n]/.test(text)) { setKey(text); setMode("exact"); }
    const selectedField = "fieldBase64" in mutation && mutation.kind !== "deleteHashField" ? mutation.fieldBase64 : undefined;
    setField(selectedField === undefined ? "" : keyCaption(selectedField, t("redis.write.emptyField")));
    void runInspect(keyBase64, redisName(keyBase64), selectedField, outcome === "applied");
  }

  async function runScan(previous?: Scan) {
    const request = ++generation.current;
    setTask("scan");
    setError(null);
    setNotice("");
    if (!previous) { setScan(null); setResult(null); }
    try {
      const batch = await scanRedisKeys(connection.workspaceId, connection.id, previous?.cursor ?? "0", pattern, connection.revision);
      if (!live.current || request !== generation.current) return;
      setScan(mergeScanBatch(previous, batch));
    } catch (cause) {
      if (live.current && request === generation.current) setError(redisErrorMessage(cause, t));
    } finally {
      if (live.current && request === generation.current) setTask(null);
    }
  }

  return <div className="flex h-full min-h-0 flex-col">
    <div className="flex flex-wrap items-center gap-2 border-b border-[var(--u-color-border)] px-3 py-2 text-xs">
      <span className="mr-auto select-text truncate">{connection.name} · {connection.host}:{connection.port} · DB {connection.dbIndex} · {redisRouteLabel(connection, t)}</span>
      {connection.allowWrites && <Button size="sm" variant="outline" onClick={() => setWrite({ kind: "createString", keyBase64: mode === "exact" ? textKey(key) : "" })}>{t("redis.write.create")}</Button>}
      <Button size="sm" variant={mode === "exact" ? "secondary" : "ghost"} aria-pressed={mode === "exact"} onClick={() => showMode("exact")}>{t("redis.exact")}</Button>
      <Button size="sm" variant={mode === "pattern" ? "secondary" : "ghost"} aria-pressed={mode === "pattern"} onClick={() => showMode("pattern")}>{t("redis.pattern")}</Button>
    </div>
    <form className="flex items-center gap-2 p-3" onSubmit={(event) => {
      event.preventDefault();
      if (mode === "exact") { setField(""); const encoded = textKey(key); void runInspect(encoded, redisName(encoded)); }
      else void runScan();
    }}>
      {mode === "exact" ?
        <Input aria-label={t("redis.key")} placeholder={t("redis.key")} value={key} onChange={(event) => { cancel(); setKey(event.target.value); setResult(null); }} /> :
        <Input aria-label={t("redis.match")} placeholder="product:*" value={pattern} onChange={(event) => { cancel(); setPattern(event.target.value); setScan(null); setResult(null); }} />}
      <Button type="submit" size="sm" disabled={task !== null}>{t(mode === "exact" ? "redis.inspect" : "redis.scan")}</Button>
      {task && <Button size="sm" variant="outline" type="button" onClick={cancel}>{t("common.confirm.cancel")}</Button>}
    </form>
    {error && <p role="alert" className="select-text px-3 text-xs text-[var(--u-color-danger)]">{error}</p>}
    {notice && !error && <p role="status" className="px-3 text-xs">{notice}</p>}
    {task && <p role="status" className="px-3 text-xs">{t("redis.reading")}</p>}
    <div className="min-h-0 flex-1 overflow-auto">
      {mode === "pattern" && scan && <section className="border-b border-[var(--u-color-border)] px-3 pb-2">
        <p role="status" className="mb-1 text-xs text-[var(--u-color-text-muted)]">{t(scan.limited ? "redis.scanLimit" : scan.cursor === "0" ? "redis.scanDone" : "redis.scanPartial", { count: scan.keys.length })}</p>
        <ul className="max-h-52 overflow-auto">
          {scan.keys.map((encoded) => {
            const label = redisName(encoded);
            return <li key={encoded}><Button className="max-w-full justify-start" size="sm" variant="ghost" onClick={() => { setField(""); void runInspect(encoded, label); }}><span className="truncate font-mono">{label}</span></Button></li>;
          })}
        </ul>
        {scan.cursor !== "0" && !scan.limited && <Button size="sm" variant="outline" disabled={task !== null} onClick={() => void runScan(scan)}>{t("redis.more")}</Button>}
      </section>}
      {result && <>
        <RedisKeyDetails key={result.encoded} result={result.info} keyLabel={result.label} readAt={result.readAt} />
        {connection.allowWrites && result.info.exists && <div className="mx-3 mb-2">
          <RedisKeyWriteMenu info={result.info} fieldBase64={result.fieldBase64} select={(kind) => setWrite({
            kind, keyBase64: result.encoded, info: result.info, fieldBase64: result.fieldBase64,
          })} />
        </div>}
        {onCreateFlow && flowKey !== null && <Button className="mx-3 mb-3" variant="outline" size="sm" onClick={() =>
          onCreateFlow({ workspaceId: connection.workspaceId, connectionId: connection.id, key: flowKey })}>{t("redis.createFlow")}</Button>}
        {result.info.keyType === "hash" && <form className="flex items-center gap-2 px-3 pb-3" onSubmit={(event) => {
          event.preventDefault(); void runInspect(result.encoded, result.label, result.fieldBase64 ?? textKey(field));
        }}>
          <Input aria-label={t("redis.hashField")} placeholder={t("redis.hashField")} value={field} disabled={task !== null} onChange={(event) => {
            setField(event.target.value);
            setError(null);
            setResult({ ...result, fieldBase64: undefined, info: { ...result.info, preview: null, fieldFound: null } });
          }} />
          <Button type="submit" size="sm" variant="outline" disabled={task !== null}>{t("redis.readField")}</Button>
        </form>}
        {result.info.exists && isRedisCollectionKind(result.info.keyType) &&
          <RedisCollectionBrowser key={result.encoded} connection={connection} kind={result.info.keyType} keyBase64={result.encoded}
            onSelectHashField={(encoded) => {
              setField(keyCaption(encoded, t("redis.write.emptyField")));
              void runInspect(result.encoded, result.label, encoded);
            }} />}
      </>}
    </div>
    {active && write && connection.allowWrites && <RedisWriteDialog connection={connection} target={write} onClose={() => setWrite(null)}
      onResolve={refreshWrittenKey} onInvalidate={() => { cancel(); setResult(null); setScan(null); }}
      onRefreshConnections={async () => { setKey(""); setField(""); await onRefreshConnections?.(); }} />}
  </div>;
}
