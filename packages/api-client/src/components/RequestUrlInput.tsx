import { useState, type RefObject } from "react";
import { Dialog, Input, useI18n } from "@unfour/ui";
import { requestTabNeedsCloseConfirmation, type ApiRequestTab } from "../model/request-tabs";
import { CurlImportContent } from "./ApiCurlImport";

export function RequestUrlInput({ tab, inputRef, onUpdate }: {
  tab: ApiRequestTab;
  inputRef?: RefObject<HTMLInputElement | null>;
  onUpdate: (patch: Partial<ApiRequestTab["draft"]>) => void;
}) {
  const { t } = useI18n();
  const [paste, setPaste] = useState<{ source: string; input: HTMLInputElement } | null>(null);
  const blank = tab.source === "new" && !tab.draft.url && !tab.sending && !tab.lastRequest &&
    !tab.sendError && !tab.execution && !tab.draft.temporaryHandle && !tab.draft.envVariables.length &&
    !requestTabNeedsCloseConfirmation(tab);
  if (paste && !blank) setPaste(null);

  return <>
    <Input
      aria-label={t("api.request.url")}
      className="h-full min-w-0 flex-1 rounded-none rounded-r-[var(--u-radius-md)] border-0 bg-transparent px-3 font-mono text-[12px] shadow-none focus:ring-0"
      onChange={(event) => onUpdate({ url: event.target.value })}
      onPaste={(event) => {
        if (!blank) return;
        const text = event.clipboardData.getData("text/plain");
        if (!/^\s*(["']?)curl(?:\.exe)?\1\s+\S/.test(text)) return;
        event.preventDefault();
        setPaste({ source: text, input: event.currentTarget });
      }}
      placeholder={t(blank ? "api.request.urlOrCurl" : "api.request.urlPlaceholder")}
      ref={inputRef}
      value={tab.draft.url}
    />
    {paste && blank && <Dialog open onOpenChange={(open) => { if (!open) setPaste(null); }}>
      <CurlImportContent
        pastedSource={paste.source}
        onClose={() => setPaste(null)}
        onImport={onUpdate}
        onReturnFocus={() => paste.input.focus()}
      />
    </Dialog>}
  </>;
}
