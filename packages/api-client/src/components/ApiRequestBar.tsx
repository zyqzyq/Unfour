import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type CSSProperties,
  type Ref,
  type RefObject,
} from "react";
import { Pencil, Save, Send, Square } from "lucide-react";
import { Button, IconButton, Input, useI18n } from "@unfour/ui";
import { httpMethods } from "../constants/http-methods";
import { requestTabTitle, type ApiRequestTab } from "../model/request-tabs";

export function ApiRequestBar({
  onNameCommit,
  onSave,
  onSend,
  onStop = () => undefined,
  onUpdate,
  tab,
  urlInputRef,
}: {
  onNameCommit: (name: string) => void;
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
      className="flex shrink-0 flex-col gap-1.5 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 py-2"
      onKeyDownCapture={(event) => {
        if (
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
        <span className="w-16 shrink-0 whitespace-nowrap text-[11px] font-semibold text-[var(--u-color-text-soft)]">
          {t("api.request.nameLabel")}
        </span>
        <RequestNameEditor
          onCommit={onNameCommit}
          ref={nameEditorRef}
          saveButtonRef={saveButtonRef}
          tab={tab}
        />
      </div>
      <div className="flex min-w-0 items-center gap-2">
        <select
          aria-label={t("api.request.method")}
          className="h-[var(--u-size-input)] shrink-0 cursor-pointer rounded-[var(--u-radius-md)] border bg-[var(--u-color-surface)] px-2.5 font-mono text-[12px] font-bold uppercase tracking-wide outline-none transition-colors duration-150 focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]"
          onChange={(event) => onUpdate({ method: event.target.value })}
          style={methodSelectStyle(tab.draft.method)}
          value={tab.draft.method}
        >
          {httpMethods.map((method) => (
            <option key={method} style={{ color: methodColor(method) }}>
              {method}
            </option>
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
        {tab.sending ? (
          <Button
            aria-label={tab.cancelling ? t("api.actions.cancelling") : t("api.actions.stop")}
            disabled={tab.cancelling}
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
            disabled={!tab.draft.url.trim()}
            size="sm"
            onClick={onSend}
            type="button"
          >
            <Send size={14} />
            {t("api.actions.send")}
          </Button>
        )}
        <Button
          aria-label={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
          disabled={tab.saving}
          ref={saveButtonRef}
          size="icon"
          onClick={handleSave}
          title={tab.saving ? t("api.actions.saving") : t("api.actions.save")}
          type="button"
          variant="outline"
        >
          <Save size={14} />
        </Button>
      </div>
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
    saveButtonRef: RefObject<HTMLButtonElement | null>;
    tab: ApiRequestTab;
  }
>(function RequestNameEditor({ onCommit, saveButtonRef, tab }, ref) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(tab.draft.name);
  const editingRef = useRef(false);

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
    setValue(tab.draft.name);
    setEditing(true);
  }

  function cancelEditing() {
    editingRef.current = false;
    setEditing(false);
    setValue(tab.draft.name);
  }

  function commitEditing() {
    if (!editingRef.current) {
      return;
    }
    editingRef.current = false;
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
        setEditing(false);
        if (tab.savedRequestId && !nextName) {
          setValue(tab.draft.name);
          return undefined;
        }

        setValue(nextName);
        return nextName;
      },
    }),
    [tab.draft.name, tab.savedRequestId, value],
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
        className="min-w-0 max-w-full shrink truncate px-2 pr-1 text-[13px] font-medium text-[var(--u-color-text)]"
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

function methodSelectStyle(method: string): CSSProperties {
  return {
    borderColor: methodBorderColor(method),
    color: methodColor(method),
  };
}

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

function methodBorderColor(method: string): string {
  switch (method.trim().toUpperCase()) {
    case "GET":
      return "color-mix(in srgb, var(--u-color-info) 40%, var(--u-color-border))";
    case "POST":
      return "color-mix(in srgb, var(--u-color-success) 40%, var(--u-color-border))";
    case "PUT":
      return "color-mix(in srgb, var(--u-color-warning) 40%, var(--u-color-border))";
    case "PATCH":
      return "color-mix(in srgb, var(--u-color-primary) 42%, var(--u-color-border))";
    case "DELETE":
      return "color-mix(in srgb, var(--u-color-danger) 40%, var(--u-color-border))";
    case "HEAD":
      return "color-mix(in srgb, var(--u-color-secondary) 40%, var(--u-color-border))";
    case "OPTIONS":
      return "color-mix(in srgb, var(--u-color-neutral) 40%, var(--u-color-border))";
    default:
      return "var(--u-color-border-strong)";
  }
}
