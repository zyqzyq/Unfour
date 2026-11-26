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
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild><IconButton label={t("api.curl.title")} size="compact" tooltipAlign="end"><Upload size={14} /></IconButton></DialogTrigger>
      {open && <CurlImportContent onClose={() => setOpen(false)} onImport={onImport} />}
    </Dialog>
  );
}

function previewCurl(source: string): { draft: RequestDraft | null; error: CurlErrorCode | null } {
  try {
    return { draft: parseCurl(source), error: null };
  } catch (failure) {
    return { draft: null, error: failure instanceof CurlImportError ? failure.code : "CURL_SYNTAX" };
  }
}

export function CurlImportContent({ pastedSource, onReturnFocus, onClose, onImport }: {
  pastedSource?: string;
  onReturnFocus?: () => void;
  onClose: () => void;
  onImport: (draft: RequestDraft) => void;
}) {
  const { t } = useI18n();
  const [source, setSource] = useState(pastedSource ?? "");
  const [{ draft, error }, setPreview] = useState(() => pastedSource ? previewCurl(pastedSource) : { draft: null, error: null });
  const showSource = !pastedSource || !draft;
  function confirm() {
    if (!draft) return;
    onImport(draft);
    onClose();
  }
  return (
    <DialogContent onCloseAutoFocus={(event) => {
      if (onReturnFocus) {
        event.preventDefault();
        onReturnFocus();
      }
    }}>
      <DialogHeader><DialogTitle>{t("api.curl.title")}</DialogTitle></DialogHeader>
      <DialogBody>
        <DialogDescription>{t(pastedSource ? "api.curl.pasteDescription" : "api.curl.description")}</DialogDescription>
        {showSource && <textarea
          aria-label={t("api.curl.source")}
          className="w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] p-2 font-mono text-[12px]"
          onChange={event => { setSource(event.target.value); setPreview({ draft: null, error: null }); }}
          rows={6} spellCheck={false} value={source}
        />}
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
        <Button variant="ghost" onClick={onClose}>{t("api.save.cancel")}</Button>
        {showSource && <Button variant={draft ? "ghost" : "default"} disabled={!source.trim()} onClick={() => setPreview(previewCurl(source))}>{t("api.curl.preview")}</Button>}
        <Button disabled={!draft} onClick={confirm}>{t(pastedSource ? "api.curl.apply" : "api.curl.confirm")}</Button>
      </DialogFooter>
    </DialogContent>
  );
}
