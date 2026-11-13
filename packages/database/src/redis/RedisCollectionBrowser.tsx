import { useEffect, useMemo, useRef, useState } from "react";
import {
  browseRedisCollection, type RedisCollectionKind, type RedisCollectionPage,
  type RedisCollectionRow, type RedisConnection,
} from "@unfour/command-client";
import { Button, DataTable, Input, Select, useI18n, type DataTableColumn } from "@unfour/ui";
import { hexBytes, readableText, redisName } from "./redis-bytes";
import { redisErrorMessage } from "./redis-error";

type Props = {
  connection: RedisConnection;
  keyBase64: string;
  kind: RedisCollectionKind;
  onSelectHashField?: (fieldBase64: string) => void;
};

export function RedisCollectionBrowser(props: Props) {
  const { connection, keyBase64, kind } = props;
  return <CollectionView key={JSON.stringify([connection.workspaceId, connection.id, connection.revision, keyBase64, kind])} {...props} />;
}

function displayBytes(base64: string, empty: string) {
  return (readableText(base64) ?? `HEX ${hexBytes(base64)}`) || empty;
}

type DisplayRow = RedisCollectionRow & { member: string; value: string };
type PageView = RedisCollectionPage & { startCursor: string; previous: string[] };
const HISTORY_LIMIT = 256;

function CollectionRows({ rows, kind, descending, onSelectHashField }: {
  rows: RedisCollectionRow[]; kind: RedisCollectionKind; descending: boolean;
  onSelectHashField: Props["onSelectHashField"];
}) {
  const { t } = useI18n();
  const displayRows = useMemo(() => rows.map((row) => ({
    ...row, member: kind === "hash" ? redisName(row.memberBase64) : displayBytes(row.memberBase64, t("redis.collection.empty")),
    value: row.valueBase64 === null ? "" : displayBytes(row.valueBase64, t("redis.collection.empty")),
  })), [kind, rows, t]);
  const columns: DataTableColumn<DisplayRow>[] = [];
  if (kind === "zset" || kind === "list") {
    const indexLabel = kind === "list" ? "index" : descending ? "reverseRank" : "rank";
    columns.push({ id: "index", header: t(`redis.collection.${indexLabel}`), width: 95, cell: (r) => r.index });
  }
  columns.push({
    id: "member", header: t(kind === "hash" ? "redis.hashField" : "redis.collection.member"), width: 230,
    cell: kind === "hash" && onSelectHashField ? (r) =>
      <Button variant="ghost" size="sm" className="max-w-full justify-start" onClick={() => onSelectHashField(r.memberBase64)}>
        <span className="truncate font-mono">{r.member}</span>
      </Button> : (r) => r.member,
  });
  if (kind === "hash") columns.push({ id: "value", header: t("redis.collection.value"), width: 300, cell: (r) => r.value });
  if (kind === "zset") columns.push({ id: "score", header: t("redis.collection.score"), width: 165, cell: (r) => r.score });
  return <DataTable className="max-h-64 select-text" columns={columns} rows={displayRows}
    empty={t("redis.collection.emptyBatch")} getRowKey={(r) => r.index ?? r.memberBase64} />;
}

function CollectionView({ connection, keyBase64, kind, onSelectHashField }: Props) {
  const { t } = useI18n();
  const scans = kind === "hash" || kind === "set";
  const [pattern, setPattern] = useState("*");
  const [index, setIndex] = useState("0");
  const [descending, setDescending] = useState(false);
  const [page, setPage] = useState<PageView | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  useEffect(() => () => { generation.current++; }, []);

  function reset() {
    generation.current++;
    setPending(false);
    setError("");
    setPage(null);
  }
  const start = Number(index);
  const invalid = !scans && (!/^\d+$/.test(index) || !Number.isSafeInteger(start));

  async function read(direction: "start" | "previous" | "next") {
    let cursor = "0";
    let previous: string[] = [];
    if (direction === "next" && page) {
      cursor = page.cursor;
      previous = [...page.previous, page.startCursor].slice(-HISTORY_LIMIT);
    } else if (direction === "previous" && page) {
      cursor = page.previous[page.previous.length - 1] ?? "0";
      previous = page.previous.slice(0, -1);
    }
    if (invalid) return;
    if (direction === "start") reset();
    const current = ++generation.current;
    setPending(true);
    setError("");
    try {
      const result = await browseRedisCollection({
        workspaceId: connection.workspaceId, connectionId: connection.id, expectedRevision: connection.revision,
        keyBase64, kind, cursor, pattern: scans ? pattern : "*", descending,
        startIndex: !scans && cursor === "0" ? start : undefined,
      });
      if (generation.current !== current) return;
      let rows = result.rows;
      if (scans) {
        const unique = new Map<string, RedisCollectionRow>();
        for (const row of rows) unique.set(row.memberBase64, row);
        rows = [...unique.values()];
      }
      const repeated = result.cursor !== "0" && (result.cursor === cursor || previous.includes(result.cursor));
      if (repeated) setError(t("redis.collection.repeatedCursor"));
      setPage({ rows, cursor: repeated ? "0" : result.cursor, startCursor: cursor, previous });
    } catch (e) {
      if (generation.current !== current) return;
      const code = e && typeof e === "object" && "code" in e ? e.code : null;
      if (code === "REDIS_KEY_CHANGED") setPage(null);
      setError(code === "REDIS_RESPONSE_TOO_LARGE"
        ? t(scans ? "redis.collection.scanLimit" : "redis.collection.rangeLimit") : redisErrorMessage(e, t));
    } finally {
      if (generation.current === current) setPending(false);
    }
  }
  return <section className="grid min-w-0 gap-2 px-3 pb-3 text-xs" aria-label={t("redis.collection.title")}>
    <form className="flex flex-wrap items-end gap-2" onSubmit={(event) => { event.preventDefault(); void read("start"); }}>
      {scans ? <label className="grid gap-1">{t(kind === "hash" ? "redis.collection.fieldPattern" : "redis.collection.memberPattern")}
        <Input value={pattern} onChange={(event) => { reset(); setPattern(event.target.value); }} /></label>
      : <label className="grid gap-1">{t(kind === "list" ? "redis.collection.startIndex" : "redis.collection.startRank")}
        <Input type="number" min={0} step={1} className="w-28" value={index} onChange={(event) => { reset(); setIndex(event.target.value); }} /></label>}
      {kind === "zset" && <label className="grid gap-1">{t("redis.collection.order")}
        <Select value={descending ? "desc" : "asc"} options={[
          { value: "asc", label: t("redis.collection.asc") }, { value: "desc", label: t("redis.collection.desc") },
        ]} onChange={(event) => { reset(); setDescending(event.target.value === "desc"); }} /></label>}
      <Button type="submit" size="sm" variant="outline" disabled={pending || invalid}>{t("redis.collection.browse")}</Button>
      {pending && <Button type="button" size="sm" variant="ghost" onClick={() => { generation.current++; setPending(false); }}>
        {t("common.confirm.cancel")}</Button>}
    </form>
    {pending && <p role="status">{t("redis.reading")}</p>}
    {error && <p role="alert" className="select-text text-[var(--u-color-danger)]">{error}</p>}
    {page && <>
      <CollectionRows rows={page.rows} kind={kind} descending={descending} onSelectHashField={onSelectHashField} />
      {(page.previous.length > 0 || page.cursor !== "0") && <div className="flex flex-wrap items-center gap-2">
        {page.previous.length > 0 && <Button size="sm" variant="outline" disabled={pending}
          onClick={() => void read("previous")}>{t("redis.collection.previous")}</Button>}
        {page.cursor !== "0" && <Button size="sm" variant="outline" disabled={pending}
          onClick={() => void read("next")}>{t("redis.collection.next")}</Button>}
      </div>}
    </>}
  </section>;
}
