import { useLayoutEffect, useRef, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { pickApiRequestFile } from "@unfour/command-client";
import { Button, Input, useI18n } from "@unfour/ui";
import type { MultipartPart } from "../model/types";

export function MultipartFormEditor({ parts, onChange }: {
  parts: MultipartPart[];
  onChange: (parts: MultipartPart[]) => void;
}) {
  const { t } = useI18n();
  const [error, setError] = useState(false);
  const [picking, setPicking] = useState<string | null>(null);
  const mounted = useRef(true);
  const latest = useRef({ parts, onChange });
  useLayoutEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useLayoutEffect(() => { latest.current = { parts, onChange }; }, [parts, onChange]);
  function update(id: string, change: (part: MultipartPart) => MultipartPart) {
    latest.current.onChange(latest.current.parts.map((part) => part.id === id ? change(part) : part));
  }
  async function pick(id: string) {
    setPicking(id);
    setError(false);
    try {
      const file = await pickApiRequestFile();
      if (file && mounted.current) update(id, (part) => part.type === "file" ? { ...part, fileName: file.name, filePath: file.path } : part);
    } catch { if (mounted.current) setError(true); }
    finally { if (mounted.current) setPicking(null); }
  }
  const cell = "min-w-0 px-1 py-1";
  return <div className="space-y-2">
    <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("api.multipart.contentType")}</p>
    {error && <p role="alert" className="text-[var(--u-color-danger)]">{t("api.multipart.pickError")}</p>}
    <div className="overflow-auto">
      <table className="w-full table-fixed text-[12px]">
        <thead><tr>
          <th className="w-10">{t("api.multipart.enabled")}</th>
          <th className="w-1/4">{t("api.multipart.key")}</th>
          <th className="w-20">{t("api.multipart.type")}</th>
          <th>{t("api.multipart.value")}</th><th className="w-16" />
        </tr></thead>
        <tbody>{parts.map((part) => <tr key={part.id}>
          <td className={cell}><input type="checkbox" aria-label={t("api.multipart.enabled")} checked={part.enabled} onChange={(event) => update(part.id, (row) => ({ ...row, enabled: event.target.checked }))} /></td>
          <td className={cell}><Input aria-label={t("api.multipart.key")} value={part.key} onChange={(event) => update(part.id, (row) => ({ ...row, key: event.target.value }))} /></td>
          <td className={cell}><select aria-label={t("api.multipart.type")} value={part.type} className="h-[var(--u-size-input)] w-full bg-[var(--u-color-bg)]" onChange={(event) => update(part.id, (row) => event.target.value === "file"
            ? { id: row.id, enabled: row.enabled, key: row.key, type: "file", fileName: null, filePath: null }
            : { id: row.id, enabled: row.enabled, key: row.key, type: "text", value: "" })}>
            <option value="text">{t("api.multipart.text")}</option><option value="file">{t("api.multipart.file")}</option>
          </select></td>
          <td className={cell}>{part.type === "text"
            ? <Input aria-label={t("api.multipart.value")} value={part.value} onChange={(event) => update(part.id, (row) => row.type === "text" ? { ...row, value: event.target.value } : row)} />
            : <div className="flex min-w-0 items-center gap-1">
              <span className="min-w-0 flex-1 truncate" title={part.fileName ?? undefined}>{part.fileName}{!part.filePath && <span className="text-[var(--u-color-text-muted)]">{part.fileName ? " · " : ""}{t("api.multipart.notSelected")}</span>}</span>
              <Button size="sm" variant="outline" disabled={picking !== null} onClick={() => void pick(part.id)}>{t("api.multipart.select")}</Button>
              {(part.fileName || part.filePath) && <Button size="sm" variant="ghost" onClick={() => update(part.id, (row) => row.type === "file" ? { ...row, fileName: null, filePath: null } : row)}>{t("api.multipart.clear")}</Button>}
            </div>}</td>
          <td className={cell}><Button size="sm" variant="ghost" aria-label={t("api.multipart.delete")} title={t("api.multipart.delete")} onClick={() => onChange(parts.filter((row) => row.id !== part.id))}><Trash2 size={13} /></Button></td>
        </tr>)}</tbody>
      </table>
    </div>
    <Button size="sm" variant="outline" onClick={() => onChange([...parts, { id: crypto.randomUUID(), enabled: true, key: "", type: "text", value: "" }])}><Plus size={13} />{t("api.multipart.add")}</Button>
  </div>;
}
