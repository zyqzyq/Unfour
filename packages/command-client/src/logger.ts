import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type FrontendLogLevel = "debug" | "info" | "warn" | "error";

export type FrontendLogEntry = {
  level: FrontendLogLevel;
  event: string;
  module: string;
  operation: string;
  fields?: unknown;
};

const REDACTED = "<redacted>";

const sensitiveKeys = new Set([
  "authorization",
  "cookie",
  "proxy-authorization",
  "x-api-key",
  "x-auth-token",
  "password",
  "passwd",
  "token",
  "access_token",
  "refresh_token",
  "secret",
  "private_key",
  "api_key",
  "license_key",
  "temporaryhandle",
  "temporaryredispassword",
]);

function isTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function isSensitiveKey(key: string) {
  return sensitiveKeys.has(key.trim().toLowerCase());
}

export function sanitizeLogValue(value: unknown, key?: string): unknown {
  if (key && isSensitiveKey(key)) {
    return REDACTED;
  }
  if (Array.isArray(value)) {
    return value.map((item) => sanitizeLogValue(item));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([entryKey, entryValue]) => [
        entryKey,
        sanitizeLogValue(entryValue, entryKey),
      ]),
    );
  }
  if (typeof value === "string") {
    return redactUrlQuery(redactSemicolonSecrets(value));
  }
  return value;
}

export async function logFrontendEvent(entry: FrontendLogEntry) {
  const safeEntry = {
    ...entry,
    fields: sanitizeLogValue(entry.fields ?? {}),
  };

  if (isTauriRuntime()) {
    try {
      await invoke("frontend_log", { entry: safeEntry });
      return;
    } catch {
      // Logging must never break the user workflow.
    }
  }

  if (import.meta.env.DEV) {
    const message = `[${safeEntry.module}] ${safeEntry.event}`;
    if (safeEntry.level === "error") {
      console.error(message, safeEntry.fields);
    } else if (safeEntry.level === "warn") {
      console.warn(message, safeEntry.fields);
    } else {
      console.debug(message, safeEntry.fields);
    }
  }
}

export function logCommandFailure(command: string, error: unknown) {
  return logFrontendEvent({
    level: "error",
    event: "tauri_command_failed",
    module: "command-client",
    operation: command,
    fields: {
      command,
      error: error instanceof Error ? error.message : String(error),
    },
  });
}

function redactUrlQuery(value: string) {
  try {
    const url = new URL(value);
    let changed = false;
    for (const key of Array.from(url.searchParams.keys())) {
      if (isSensitiveKey(key)) {
        url.searchParams.set(key, REDACTED);
        changed = true;
      }
    }
    return changed
      ? url.toString().split("%3Credacted%3E").join(REDACTED)
      : value;
  } catch {
    return value;
  }
}

function redactSemicolonSecrets(value: string) {
  if (!value.includes("=") || !value.includes(";")) return value;
  return value
    .split(";")
    .map((segment) => {
      const [key, ...rest] = segment.split("=");
      if (!rest.length || !isSensitiveKey(key)) return segment;
      return `${key}=${REDACTED}`;
    })
    .join(";");
}
