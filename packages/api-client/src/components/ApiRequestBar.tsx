import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type Ref,
  type RefObject,
} from "react";
import { Pencil, Save, Send, Square } from "lucide-react";
import { Button, IconButton, Input, Select, useI18n } from "@unfour/ui";
import { httpMethods } from "../constants/http-methods";
import { requestTabTitle, type ApiRequestTab } from "../model/request-tabs";

export function ApiRequestBar({
  onNameCommit,
  onNameEditingChange,
  onSave,
  onSend,
  onStop = () => undefined,
  onUpdate,
  tab,
  urlInputRef,
}: {
  onNameCommit: (name: string) => void;
  onNameEditingChange?: (editing: boolean) => void;
  onSave: (name?: string) => void;
  onSend: () => void;
  onStop?: () => void;
  onUpdate: (patch: Partial<ApiRequestTab["draft"]>) => void;
  tab: ApiRequestTab;
  urlInputRef?: Ref<HTMLInputElement>;
}) {
  const { t } = useI18n();
  const nameEditorRef = useRef<RequestNameEditorHandle>(null);
  const saveButtonRef = useRef<HTMLButtonElement>(null);

  function handleSave() {
    onSave(nameEditorRef.current?.takePendingName());
  }

  return (
    <div
      className="flex shrink-0 flex-col gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-canvas)] px-4 py-3"
      onKeyDown={(event) => {
        if (
          !event.defaultPrevented && !event.nativeEvent.isComposing &&
          event.nativeEvent.keyCode !== 229 && !event.repeat &&
          !event.altKey && !event.shiftKey &&
          (event.ctrlKey || event.metaKey) &&
          event.key.toLowerCase() === "s"
        ) {
          event.preventDefault();
          event.stopPropagation();
          handleSave();
        }
      }}
    >
      <div className="flex min-w-0 items-center gap-2">
        <RequestNameEditor
          onCommit={onNameCommit}
          onEditingChange={onNameEditingChange}
          ref={nameEditorRef}
          saveButtonRef={saveButtonRef}
          tab={tab}
        />
      </div>
      <div className="flex min-w-0 items-center gap-2">
        <div className="flex h-[38px] min-w-0 flex-1 items-center rounded-[var(--u-radius-md)] border border-[var(--u-color-input)] bg-[var(--u-color-canvas)] transition-colors focus-within:border-[var(--u-color-focus)] focus-within:ring-2 focus-within:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]">
          <Select
            aria-label={t("api.request.method")}
            className="h-full w-auto shrink-0 cursor-pointer rounded-none rounded-l-[var(--u-radius-md)] border-0 bg-transparent px-2.5 font-mono text-[12px] font-bold uppercase tracking-wide focus:ring-0 focus-visible:outline-offset-[-3px]"
            onChange={(event) => onUpdate({ method: event.target.value })}
            style={{ color: methodColor(tab.draft.method) }}
            value={tab.draft.method}
          >
            {httpMethods.map((method) => (
              <option key={method} style={{ color: methodColor(method) }}>
                {method}
              </option>
            ))}
          </Select>
          <span aria-hidden="true" className="h-4 w-px shrink-0 bg-[var(--u-color-border)]" />
          <Input
            aria-label={t("api.request.url")}
            className="h-full min-w-0 flex-1 rounded-none rounded-r-[var(--u-radius-md)] border-0 bg-transparent px-3 font-mono text-[12px] shadow-none focus:ring-0"
            onChange={(event) => onUpdate({ url: event.target.value })}
            placeholder={t("api.request.urlPlaceholder")}
            ref={urlInputRef}
            value={tab.draft.url}
          />
        </div>
        {tab.sending ? (
          <Button
            aria-label={tab.cancelling ? t("api.actions.cancelling") : t("api.actions.stop")}
            disabled={tab.cancelling}
            className="h-[38px] px-4"
            size="sm"
            onClick={onStop}
            type="button"
            variant="danger"
          >
            <Square fill="currentColor" size={12} />
            {tab.cancelling ? t("api.actions.cancelling") : t("api.actions.stop")}
          </Button>
        ) : (
          <Button
            className="h-[38px] px-4"
            disabled={!tab.draft.url.trim() || (tab.source === "history" && !tab.draft.historyReviewConfirmed)}
            size="sm"
            onClick={onSend}
            type="button"
          >
            <Send size={14} />
            {t(tab.source === "history" ? "api.history.sendNew" : "api.actions.send")}
          </Button>
        )}
        <Button
          aria-label={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
          className="h-[38px] w-[38px]"
          disabled={tab.saving || Boolean(tab.externalChange)}
          ref={saveButtonRef}
          size="icon"
          onClick={handleSave}
          title={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
          type="button"
          variant="ghost"
        >
          <Save size={14} />
        </Button>
      </div>
      {tab.source === "history" && (
        <div className="text-[12px] text-[var(--u-color-text-muted)]">
          <p>{t("api.history.snapshotNotice")}</p>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={Boolean(tab.draft.historyReviewConfirmed)}
              onChange={(event) => onUpdate({ historyReviewConfirmed: event.target.checked })} />
            {t("api.history.reviewConfirmed")}
          </label>
        </div>
      )}
      {tab.saveError && (
        <span
          className="min-w-0 truncate text-[12px] text-[var(--u-color-danger)]"
          title={tab.saveError}
        >
          {tab.saveError}
        </span>
      )}
    </div>
  );
}

type RequestNameEditorHandle = {
  takePendingName: () => string | undefined;
};

const RequestNameEditor = forwardRef<
  RequestNameEditorHandle,
  {
    onCommit: (name: string) => void;
    onEditingChange?: (editing: boolean) => void;
    saveButtonRef: RefObject<HTMLButtonElement | null>;
    tab: ApiRequestTab;
  }
>(function RequestNameEditor({ onCommit, onEditingChange, saveButtonRef, tab }, ref) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(tab.draft.name);
  const editingRef = useRef(false);
  const editingChangeRef = useRef(onEditingChange);
  useEffect(() => { editingChangeRef.current = onEditingChange; }, [onEditingChange]);
  useEffect(() => () => {
    if (editingRef.current) editingChangeRef.current?.(false);
  }, []);

  useEffect(() => {
    if (!editingRef.current) {
      setValue(tab.draft.name);
    }
  }, [tab.draft.name]);

  const displayName = requestTabTitle(tab, t("api.request.untitled"));

  function beginEditing() {
    if (tab.saving) {
      return;
    }
    editingRef.current = true;
    onEditingChange?.(true);
    setValue(tab.draft.name);
    setEditing(true);
  }

  function cancelEditing() {
    editingRef.current = false;
    onEditingChange?.(false);
    setEditing(false);
    setValue(tab.draft.name);
  }

  function commitEditing() {
    if (!editingRef.current) {
      return;
    }
    editingRef.current = false;
    onEditingChange?.(false);
    setEditing(false);

    const nextName = value.trim();
    if (tab.savedRequestId && !nextName) {
      setValue(tab.draft.name);
      return;
    }

    setValue(nextName);
    if (nextName !== tab.draft.name.trim()) {
      onCommit(nextName);
    }
  }

  useImperativeHandle(
    ref,
    () => ({
      takePendingName() {
        if (!editingRef.current) {
          return undefined;
        }

        const nextName = value.trim();
        editingRef.current = false;
        onEditingChange?.(false);
        setEditing(false);
        if (tab.savedRequestId && !nextName) {
          setValue(tab.draft.name);
          return undefined;
        }

        setValue(nextName);
        return nextName;
      },
    }),
    [tab.draft.name, tab.savedRequestId, value, onEditingChange],
  );

  return editing ? (
    <Input
      aria-label={t("api.request.name")}
      autoFocus
      className="min-w-0 flex-1"
      maxLength={120}
      onBlur={(event) => {
        if (event.relatedTarget === saveButtonRef.current) {
          return;
        }
        commitEditing();
      }}
      onChange={(event) => setValue(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          commitEditing();
        }
        if (event.key === "Escape") {
          event.preventDefault();
          cancelEditing();
        }
      }}
      placeholder={t("api.request.untitled")}
      value={value}
    />
  ) : (
    <div className="flex min-w-0 flex-1 items-center gap-0.5">
      <span
        className="min-w-0 max-w-full shrink truncate pr-1 text-[13px] font-medium text-[var(--u-color-text)]"
        title={displayName}
      >
        {displayName}
      </span>
      <IconButton
        disabled={tab.saving}
        label={t("api.request.editName")}
        onClick={beginEditing}
        size="compact"
        tooltip={t("api.request.editName")}
      >
        <Pencil size={13} />
      </IconButton>
    </div>
  );
});

function methodColor(method: string): string {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "var(--u-color-info-text)";
    case "POST":
      return "var(--u-color-success)";
    case "PUT":
      return "var(--u-color-warning-text)";
    case "PATCH":
      return "var(--u-color-primary)";
    case "DELETE":
      return "var(--u-color-danger-text)";
    case "HEAD":
      return "var(--u-color-secondary-text)";
    case "OPTIONS":
      return "var(--u-color-neutral-text)";
    default:
      return "var(--u-color-text-muted)";
  }
}
