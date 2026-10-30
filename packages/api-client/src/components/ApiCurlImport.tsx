import { useState } from "react";
import { Upload } from "lucide-react";
import { Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger, IconButton, Input, SecretInput, useI18n } from "@unfour/ui";
import { CurlImportError, parseCurl, type CurlErrorCode } from "../model/curl-import";
import type { RequestDraft } from "../model/types";
import { isSensitiveField } from "../model/sensitive-field";
import { stripUrlQuery } from "../request-utils";

export function ApiCurlImport({ onImport }: { onImport: (draft: RequestDraft) => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [source, setSource] = useState("");
  const [draft, setDraft] = useState<RequestDraft | null>(null);
  const [error, setError] = useState<CurlErrorCode | null>(null);
  function close() { setOpen(false); setSource(""); setDraft(null); setError(null); }
  function preview() {
    try { setDraft(parseCurl(source)); setError(null); }
    catch (failure) { setDraft(null); setError(failure instanceof CurlImportError ? failure.code : "CURL_SYNTAX"); }
  }
  return <>
    <Dialog open={open} onOpenChange={next => next ? setOpen(true) : close()}>
      <DialogTrigger asChild><IconButton label={t("api.curl.title")} size="compact"><Upload size={14} /></IconButton></DialogTrigger>
      <DialogContent title={t("api.curl.title")}>
        <DialogHeader><DialogTitle>{t("api.curl.title")}</DialogTitle></DialogHeader>
        <DialogBody>
          <DialogDescription>{t("api.curl.description")}</DialogDescription>
          <textarea className="w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] p-2 font-mono text-[12px]" aria-label={t("api.curl.source")} value={source} rows={6} spellCheck={false} onChange={event => { setSource(event.target.value); setDraft(null); setError(null); }} />
          {error && <p role="alert">{t(`api.curl.errors.${error}`)}</p>}
          {draft && <div className="space-y-2">
            <label className="block">{t("api.curl.method")}<Input value={draft.method} readOnly /></label>
            <label className="block">{t("api.curl.url")}<Input value={stripUrlQuery(draft.url)} readOnly /></label>
            {(["query", "headers"] as const).map(kind => <section key={kind} aria-label={t(`api.curl.${kind}`)}>
              <p>{t(`api.curl.${kind}`)}</p>
              {draft[kind].map((row, index) => <div key={index} className="flex gap-2">
                <Input aria-label={t("api.keyValue.key")} value={row.key} readOnly />
                {isSensitiveField(row.key) ? <SecretInput aria-label={row.key} value={row.value} readOnly /> : <Input aria-label={row.key} value={row.value} readOnly />}
              </div>)}
            </section>)}
            <textarea className="w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] p-2 font-mono text-[12px]" aria-label={t("api.curl.body")} value={draft.body} readOnly rows={4} />
          </div>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={close}>{t("api.save.cancel")}</Button>
          <Button variant={draft ? "ghost" : "default"} disabled={!source.trim()} onClick={preview}>{t("api.curl.preview")}</Button>
          <Button disabled={!draft} onClick={() => { if (draft) { onImport(draft); close(); } }}>{t("api.curl.confirm")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </>;
}
