import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { exportDatabaseTable, type DatabaseConnection, type DatabaseExportTableInput, type DatabaseTable } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle, Select, useI18n } from "@unfour/ui";

export function TableExportDialog({ connection, table, onOpenChange }: {
  connection: DatabaseConnection;
  table: DatabaseTable;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const [content, setContent] = useState<DatabaseExportTableInput["content"]>("structure-and-data");
  const [format, setFormat] = useState<DatabaseExportTableInput["format"]>("sql");
  const [path, setPath] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  async function chooseLocation() {
    try {
      const selected = await saveFileDialog({ defaultPath: `${table.name}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
      if (selected) setPath(selected);
    } catch (cause) { setError(String(cause)); }
  }

  async function exportTable() {
    if (!path) return;
    setPending(true);
    setError(null);
    try {
      const exported = await exportDatabaseTable({
        workspaceId: connection.workspaceId, connectionId: connection.id,
        catalog: table.catalog, schema: table.schema, tableName: table.name,
        content, format, destinationPath: path,
      });
      setResult(t("database.export.done", { rows: exported.rowCount, bytes: exported.bytesWritten }));
    } catch (cause) { setError(String(cause)); }
    finally { setPending(false); }
  }

  return (
    <Dialog onOpenChange={(open) => !pending && onOpenChange(open)} open>
      <DialogContent title={t("database.export.title")}>
        <DialogHeader><DialogTitle>{t("database.export.title")}</DialogTitle></DialogHeader>
        <DialogBody className="space-y-3">
          <label className="block space-y-1 text-[12px]">
            <span>{t("database.export.content")}</span>
            <Select aria-label={t("database.export.content")} onChange={(event) => {
              const next = event.target.value as DatabaseExportTableInput["content"];
              setContent(next);
              if (next !== "data") { setFormat("sql"); setPath(null); }
            }} options={[
              { label: t("database.export.structure"), value: "structure" },
              { label: t("database.export.data"), value: "data" },
              { label: t("database.export.both"), value: "structure-and-data" },
            ]} value={content} />
          </label>
          <label className="block space-y-1 text-[12px]">
            <span>{t("database.export.format")}</span>
            <Select aria-label={t("database.export.format")} onChange={(event) => { setFormat(event.target.value as DatabaseExportTableInput["format"]); setPath(null); }} options={[
              { label: "SQL", value: "sql" },
              ...(content === "data" ? [{ label: "CSV", value: "csv" }, { label: "JSON", value: "json" }] : []),
            ]} value={format} />
          </label>
          <div className="space-y-1 text-[12px]">
            <span>{t("database.export.location")}</span>
            <div className="flex items-center gap-2"><Button onClick={() => void chooseLocation()} size="sm" type="button" variant="outline">{t("database.export.choose")}</Button>
              <span className="min-w-0 truncate" title={path ?? undefined}>{path ?? t("database.export.noLocation")}</span>
            </div>
          </div>
          {error ? <p className="text-[12px] text-[var(--u-color-danger)]" role="alert">{error}</p> : null}
          {result ? <p className="text-[12px] text-[var(--u-color-text)]" role="status">{result}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button disabled={pending} onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">{t("common.confirm.cancel")}</Button>
          <Button disabled={!path || pending} onClick={() => void exportTable()} size="sm" type="button">{t("database.export.export")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
