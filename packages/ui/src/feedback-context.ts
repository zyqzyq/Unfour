import * as React from "react";
import { useI18n } from "./i18n";

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

export const FeedbackContext = React.createContext<FeedbackApi | null>(null);

const noopFeedback: FeedbackApi = {
  error: () => undefined,
  info: () => undefined,
  show: () => undefined,
  success: () => undefined,
};

export function useFeedback(): FeedbackApi {
  const ctx = React.useContext(FeedbackContext);
  return ctx ?? noopFeedback;
}

/**
 * Header/field names whose values must never reach logs, history, or
 * local activity details (per project security rules).
 */
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
  /** Resolved through the i18n translator. Used as the toast title when provided. */
  key?: string;
  /** Explicit title; used when no `key` is provided. */
  message?: string;
}

/** Pull a human-readable detail string out of Error / string / Tauri AppError payloads. */
export function extractErrorDetail(error: unknown): string | undefined {
  if (error instanceof Error && error.message.trim()) {
    return cleanErrorDetail(error.message);
  }
  if (typeof error === "string" && error.trim()) {
    return cleanErrorDetail(error);
  }
  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    if (typeof record.message === "string" && record.message.trim()) {
      return cleanErrorDetail(record.message);
    }
  }
  return undefined;
}

function cleanErrorDetail(message: string) {
  return message
    .replace(
      /^(validation|configuration|database|http|io|serialization|unsupported|read-only|timeout) error:\s*/i,
      "",
    )
    .replace(/^not found:\s*/i, "")
    .trim();
}

/**
 * Returns a stable `onError`-style handler that surfaces an operation failure
 * to the user via the feedback toast and logs a redacted diagnostic. Intended
 * for react-query `onError` callbacks and promise `.catch` blocks so failures
 * are never silently swallowed.
 *
 * When a fallback title is provided, the underlying backend detail is shown as
 * the toast description so users can see *why* the action failed.
 */
export function useFeedbackErrorHandler() {
  const feedback = useFeedback();
  const { t } = useI18n();
  return React.useCallback(
    (error: unknown, fallback?: FeedbackErrorFallback) => {
      const detail = extractErrorDetail(error);
      const title =
        fallback?.message ??
        (fallback?.key ? t(fallback.key) : undefined) ??
        detail ??
        t("feedback.error.default");
      const description =
        detail && detail !== title
          ? detail
          : undefined;
      feedback.error(title, {
        description,
        durationMs: description ? 8_000 : undefined,
      });
      console.error("[unfour] operation failed:", redactForLog(error));
    },
    [feedback, t],
  );
}
