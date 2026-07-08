import * as React from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useI18n } from "./i18n";
import { cn } from "./utils";

export type FeedbackTone = "success" | "error" | "info";

export interface FeedbackOptions {
  description?: string;
  durationMs?: number;
}

export interface FeedbackApi {
  success: (message: string, options?: FeedbackOptions) => void;
  error: (message: string, options?: FeedbackOptions) => void;
  info: (message: string, options?: FeedbackOptions) => void;
  show: (tone: FeedbackTone, message: string, options?: FeedbackOptions) => void;
}

interface FeedbackItem {
  description?: string;
  id: string;
  message: string;
  tone: FeedbackTone;
}

const DEFAULT_DURATION_MS = 4500;

const FeedbackContext = React.createContext<FeedbackApi | null>(null);

const noopFeedback: FeedbackApi = {
  error: () => undefined,
  info: () => undefined,
  show: () => undefined,
  success: () => undefined,
};

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

// eslint-disable-next-line react-refresh/only-export-components -- hook paired with its provider; splitting would create a circular import
export function useFeedback(): FeedbackApi {
  const ctx = React.useContext(FeedbackContext);
  return ctx ?? noopFeedback;
}

/**
 * Header/field names whose values must never reach logs, history, or
 * local activity details (per project security rules).
 */
// eslint-disable-next-line react-refresh/only-export-components -- shared helper exported alongside the provider component
export const SENSITIVE_KEYS = [
  "authorization",
  "cookie",
  "proxy-authorization",
  "x-api-key",
  "x-auth-token",
] as const;

/**
 * Produce a log-safe string from an arbitrary error/value, replacing the
 * values of sensitive header-like keys with `[REDACTED]`. Guards against
 * circular structures so it can be called on any thrown value.
 */
// eslint-disable-next-line react-refresh/only-export-components -- shared helper exported alongside the provider component
export function redactForLog(value: unknown): string {
  try {
    const clone = redactNode(value, new WeakSet());
    return JSON.stringify(clone, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

function redactNode(node: unknown, seen: WeakSet<object>): unknown {
  if (node === null || typeof node !== "object") {
    return node;
  }
  if (seen.has(node)) {
    return "[Circular]";
  }
  seen.add(node);
  if (Array.isArray(node)) {
    return node.map((item) => redactNode(item, seen));
  }
  const out: Record<string, unknown> = {};
  for (const [key, val] of Object.entries(node as Record<string, unknown>)) {
    if (SENSITIVE_KEYS.includes(key.toLowerCase() as (typeof SENSITIVE_KEYS)[number])) {
      out[key] = "[REDACTED]";
    } else {
      out[key] = redactNode(val, seen);
    }
  }
  return out;
}

export interface FeedbackErrorFallback {
  /** Resolved through the i18n translator. Takes precedence over `message`. */
  key?: string;
  /** Explicit message; used when no `key` is provided. */
  message?: string;
}

/**
 * Returns a stable `onError`-style handler that surfaces an operation failure
 * to the user via the feedback toast and logs a redacted diagnostic. Intended
 * for react-query `onError` callbacks and promise `.catch` blocks so failures
 * are never silently swallowed.
 */
// eslint-disable-next-line react-refresh/only-export-components -- hook paired with its provider; splitting would create a circular import
export function useFeedbackErrorHandler() {
  const feedback = useFeedback();
  const { t } = useI18n();
  return React.useCallback(
    (error: unknown, fallback?: FeedbackErrorFallback) => {
      const detail =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : undefined;
      const message =
        fallback?.message ??
        (fallback?.key ? t(fallback.key) : undefined) ??
        detail ??
        t("feedback.error.default");
      feedback.error(message);
      console.error("[unfour] operation failed:", redactForLog(error));
    },
    [feedback, t],
  );
}
