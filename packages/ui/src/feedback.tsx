import { FeedbackContext, type FeedbackApi, type FeedbackOptions, type FeedbackTone } from "./feedback-context";
import * as React from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useI18n } from "./i18n";
import { cn } from "./utils";

interface FeedbackItem {
  description?: string;
  id: string;
  message: string;
  tone: FeedbackTone;
}

const DEFAULT_DURATION_MS = 4500;

export function FeedbackProvider({
  children,
  defaultDurationMs = DEFAULT_DURATION_MS,
}: {
  children: React.ReactNode;
  defaultDurationMs?: number;
}) {
  const [items, setItems] = React.useState<FeedbackItem[]>([]);
  const timers = React.useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const remove = React.useCallback((id: string) => {
    setItems((current) => current.filter((item) => item.id !== id));
    const timer = timers.current.get(id);
    if (timer) {
      clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const show = React.useCallback(
    (tone: FeedbackTone, message: string, options?: FeedbackOptions) => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      const item: FeedbackItem = {
        description: options?.description,
        id,
        message,
        tone,
      };
      setItems((current) => [...current, item]);
      const duration = options?.durationMs ?? defaultDurationMs;
      const timer = setTimeout(() => remove(id), duration);
      timers.current.set(id, timer);
    },
    [defaultDurationMs, remove],
  );

  const api = React.useMemo<FeedbackApi>(
    () => ({
      error: (message, options) => show("error", message, options),
      info: (message, options) => show("info", message, options),
      show,
      success: (message, options) => show("success", message, options),
    }),
    [show],
  );

  React.useEffect(() => {
    const map = timers.current;
    return () => {
      map.forEach((timer) => clearTimeout(timer));
      map.clear();
    };
  }, []);

  return (
    <FeedbackContext.Provider value={api}>
      {children}
      <div
        aria-live="polite"
        className="pointer-events-none fixed bottom-4 right-4 z-[9999] flex w-[340px] max-w-[calc(100vw-2rem)] flex-col gap-2"
      >
        {items.map((item) => (
          <FeedbackToast
            key={item.id}
            item={item}
            onDismiss={() => remove(item.id)}
          />
        ))}
      </div>
    </FeedbackContext.Provider>
  );
}

const TONE_STYLES: Record<
  FeedbackTone,
  { accent: string; icon: React.ReactNode }
> = {
  success: {
    accent: "var(--u-color-success)",
    icon: <CheckCircle2 size={16} />,
  },
  error: {
    accent: "var(--u-color-danger)",
    icon: <XCircle size={16} />,
  },
  info: {
    accent: "var(--u-color-info)",
    icon: <Info size={16} />,
  },
};

function FeedbackToast({
  item,
  onDismiss,
}: {
  item: FeedbackItem;
  onDismiss: () => void;
}) {
  const { t } = useI18n();
  const tone = TONE_STYLES[item.tone];

  return (
    <div
      className="pointer-events-auto flex items-start gap-2 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] border-l-4 bg-[var(--u-color-surface)] p-3 text-[13px] shadow-[var(--u-shadow-md)]"
      role={item.tone === "error" ? "alert" : "status"}
      style={{ borderLeftColor: tone.accent }}
    >
      <span
        className="mt-0.5 shrink-0"
        style={{ color: tone.accent }}
      >
        {tone.icon}
      </span>
      <div className="min-w-0 flex-1">
        <p className="font-medium text-[var(--u-color-text)]">{item.message}</p>
        {item.description && (
          <p className="mt-0.5 break-words text-[12px] text-[var(--u-color-text-muted)]">
            {item.description}
          </p>
        )}
      </div>
      <button
        aria-label={t("feedback.dismiss")}
        className={cn(
          "shrink-0 rounded p-0.5 text-[var(--u-color-text-soft)] transition-colors",
          "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
        )}
        onClick={onDismiss}
        type="button"
      >
        <X size={14} />
      </button>
    </div>
  );
}
