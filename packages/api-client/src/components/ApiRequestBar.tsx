import type { Ref } from "react";
import { Save, Send } from "lucide-react";
import { Button, Input, cn, useI18n } from "@unfour/ui";
import type { ApiRequestTab } from "../model/request-tabs";
import { httpMethods } from "../constants/http-methods";
import { RequestActionsMenu } from "./RequestActionsMenu";

export function ApiRequestBar({
  onDelete,
  onDuplicate,
  onExport,
  onImport,
  onSave,
  onSend,
  onUpdate,
  tab,
  urlInputRef,
}: {
  onDelete: () => void;
  onDuplicate: () => void;
  onExport: () => void;
  onImport: () => void;
  onSave: () => void;
  onSend: () => void;
  onUpdate: (patch: Partial<ApiRequestTab["draft"]>) => void;
  tab: ApiRequestTab;
  urlInputRef?: Ref<HTMLInputElement>;
}) {
  const { t } = useI18n();

  return (
    <div className="flex min-h-[52px] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 py-2.5">
      <select
        aria-label={t("api.request.method")}
        className={cn(
          "h-[var(--u-size-input)] shrink-0 cursor-pointer rounded-[var(--u-radius-md)] border bg-[var(--u-color-surface)] px-2.5 font-mono text-[12px] font-bold uppercase tracking-wide outline-none transition-colors duration-150 focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]",
          methodSelectToneClass(tab.draft.method),
        )}
        onChange={(event) => onUpdate({ method: event.target.value })}
        value={tab.draft.method}
      >
        {httpMethods.map((method) => (
          <option key={method}>{method}</option>
        ))}
      </select>
      <Input
        aria-label={t("api.request.url")}
        className="min-w-0 flex-1 rounded-[var(--u-radius-md)] border-[var(--u-color-border-strong)] bg-[var(--u-color-surface)] font-mono text-[12px]"
        onChange={(event) => onUpdate({ url: event.target.value })}
        placeholder={t("api.request.urlPlaceholder")}
        ref={urlInputRef}
        value={tab.draft.url}
      />
      <Button
        disabled={tab.sending || !tab.draft.url.trim()}
        size="sm"
        onClick={onSend}
        type="button"
      >
        <Send size={14} />
        {tab.sending ? t("api.actions.sending") : t("api.actions.send")}
      </Button>
      <Button
        aria-label={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
        disabled={tab.saving}
        size="icon"
        onClick={onSave}
        title={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
        type="button"
        variant="outline"
      >
        <Save size={14} />
      </Button>
      <RequestActionsMenu
        canDelete={Boolean(tab.savedRequestId)}
        canDuplicate={Boolean(tab.savedRequestId)}
        onDelete={onDelete}
        onDuplicate={onDuplicate}
        onExport={onExport}
        onImport={onImport}
      />
      {tab.saveError && (
        <span
          className="max-w-[180px] truncate text-[12px] text-[var(--u-color-danger)]"
          title={tab.saveError}
        >
          {tab.saveError}
        </span>
      )}
    </div>
  );
}

function methodSelectToneClass(method: string) {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "border-[color:color-mix(in_srgb,var(--u-color-info)_40%,var(--u-color-border))] text-[var(--u-color-info-text)]";
    case "POST":
      return "border-[color:color-mix(in_srgb,var(--u-color-success)_40%,var(--u-color-border))] text-[var(--u-color-success)]";
    case "PUT":
      return "border-[color:color-mix(in_srgb,var(--u-color-warning)_40%,var(--u-color-border))] text-[var(--u-color-warning-text)]";
    case "PATCH":
      return "border-[color:color-mix(in_srgb,var(--u-color-primary)_42%,var(--u-color-border))] text-[var(--u-color-primary)]";
    case "DELETE":
      return "border-[color:color-mix(in_srgb,var(--u-color-danger)_40%,var(--u-color-border))] text-[var(--u-color-danger-text)]";
    default:
      return "border-[var(--u-color-border-strong)] text-[var(--u-color-text-muted)]";
  }
}
