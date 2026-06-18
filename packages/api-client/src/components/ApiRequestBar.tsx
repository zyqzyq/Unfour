import type { Ref } from "react";
import { Save, Send } from "lucide-react";
import { Button, Input, cn, useI18n } from "@unfour/ui";
import {
  methodBadgeToneClass,
  type ApiRequestTab,
} from "../model/request-tabs";
import { methods } from "../hooks/useApiRequest";
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
    <div className="flex min-h-[48px] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 py-2">
      <div className="flex min-w-0 flex-1 items-stretch gap-1.5 overflow-hidden rounded-[var(--u-radius-md)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] p-1 focus-within:border-[var(--u-color-focus)] focus-within:ring-1 focus-within:ring-[color:color-mix(in_srgb,var(--u-color-focus)_30%,transparent)]">
        <select
          aria-label="HTTP method"
          className={cn(
            "h-[26px] shrink-0 cursor-pointer rounded-[var(--u-radius-sm)] border-0 px-2 text-[12px] font-bold uppercase tracking-wide outline-none",
            methodBadgeToneClass(tab.draft.method),
          )}
          onChange={(event) => onUpdate({ method: event.target.value })}
          value={tab.draft.method}
        >
          {methods.map((method) => (
            <option key={method}>{method}</option>
          ))}
        </select>
        <Input
          aria-label="Request URL"
          className="min-w-0 flex-1 border-0 bg-transparent font-mono focus:border-0 focus:ring-0"
          onChange={(event) => onUpdate({ url: event.target.value })}
          placeholder="https://api.example.com/resource"
          ref={urlInputRef}
          value={tab.draft.url}
        />
      </div>
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
        disabled={tab.saving}
        size="sm"
        onClick={onSave}
        type="button"
        variant="outline"
      >
        <Save size={14} />
        {tab.saving ? t("api.actions.saving") : t("api.actions.save")}
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
