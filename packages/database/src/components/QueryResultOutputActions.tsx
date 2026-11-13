import { Clipboard, Download, FileDown, FileJson } from "lucide-react";
import { useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import {
  Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
  IconButton, Select, useI18n,
} from "@unfour/ui";
import { serializeDatabaseResult, serializeDatabaseResultJson } from "../result-utils";
import type { QueryResultView } from "./query-result-view";

function downloadResult(content: string, mime: string, extension: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `unfour-query-results-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.${extension}`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

export function QueryResultOutputActions({ result, view }: {
  result: DatabaseQueryResult | null;
  view: QueryResultView;
}) {
  const { t } = useI18n();
  const [scope, setScope] = useState<"view" | "all">("view");
  const [lastResult, setLastResult] = useState(result);
  if (result !== lastResult) {
    setLastResult(result);
    setScope("view");
  }
  const [feedback, setFeedback] = useState<{
    rows: DatabaseQueryResult["rows"]; status: "copied" | "failed";
  } | null>(null);
  const filtered = Boolean(view.filter.trim());
  const hasView = filtered || Boolean(view.sort);
  const rows = scope === "view" && hasView ? view.rows : result?.rows;
  const output = result?.columns.length && rows ? { ...result, rows } : null;
  const copyStatus = feedback && feedback.rows === rows ? feedback.status : null;

  async function copyTsv() {
    if (!output) return;
    let status: "copied" | "failed";
    try {
      await navigator.clipboard.writeText(serializeDatabaseResult(output, "\t"));
      status = "copied";
    } catch {
      status = "failed";
    }
    const updated = { rows: output.rows, status };
    setFeedback(updated);
    window.setTimeout(() => setFeedback((current) => current === updated ? null : current), 1600);
  }

  function exportCsv() {
    if (output) downloadResult(serializeDatabaseResult(output, ","), "text/csv;charset=utf-8", "csv");
  }

  function exportJson() {
    if (output) downloadResult(serializeDatabaseResultJson(output), "application/json;charset=utf-8", "json");
  }

  return <>
    {result && hasView && (
      <Select
        aria-label={t("database.result.outputScope")}
        className="h-7 w-auto max-w-full text-[12px]"
        onChange={(event) => setScope(event.target.value === "all" ? "all" : "view")}
        value={scope}
      >
        <option value="view">
          {t(filtered ? "database.result.filteredScope" : "database.result.sortedScope", { count: view.rows.length })}
        </option>
        <option value="all">{t("database.result.returnedScope", { count: result.rows.length })}</option>
      </Select>
    )}
    <Button disabled={!output} onClick={copyTsv} size="sm" type="button" variant="outline">
      <Clipboard size={13} />
      {t(copyStatus === "copied" ? "database.result.copied" : copyStatus === "failed" ? "database.result.copyFailed" : "database.result.copy")}
    </Button>
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton disabled={!output} label={t("database.result.export")}>
          <Download size={13} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onSelect={exportCsv}>
          <FileDown size={13} />
          {t("database.result.exportCsv")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={exportJson}>
          <FileJson size={13} />
          {t("database.result.exportJson")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </>;
}
