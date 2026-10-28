import { invoke } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { AccountState, AccountStateSnapshot } from "./accountTypes";

const processedDeepLinks = new Set<string>();
const inFlightDeepLinks = new Map<string, Promise<AccountStateSnapshot>>();
const MAX_PROCESSED_LINKS = 64;

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined"
    && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

export function getAccountState(): Promise<AccountStateSnapshot> {
  return invoke<AccountStateSnapshot>("account_get_state");
}

export function beginAccountSignIn(): Promise<AccountState> {
  return invoke<AccountState>("account_begin_sign_in");
}

export function signOutAccount(): Promise<AccountState> {
  return invoke<AccountState>("account_sign_out");
}

export function openAccountUpgrade(): Promise<void> {
  return invoke<void>("account_open_upgrade");
}

export function openWebAccount(): Promise<void> {
  return invoke<void>("account_open_web_account");
}

export function getAccountCommandErrorCode(error: unknown): string | null {
  if (typeof error !== "object" || error === null || !("code" in error)) return null;
  const code = (error as { code?: unknown }).code;
  return typeof code === "string" ? code : null;
}

async function processDeepLink(url: string): Promise<AccountStateSnapshot | null> {
  let parsed: URL;
  try { parsed = new URL(url); } catch { return null; }
  if (parsed.protocol !== "unfour:" || parsed.hostname !== "auth" || parsed.pathname !== "/callback") return null;
  // Keep only bounded digests in the replay cache, never raw codes or state.
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(url));
  const key = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");

  const existing = inFlightDeepLinks.get(key);
  if (existing) return existing;
  if (processedDeepLinks.has(key)) return null;

  // Failed exchanges also count as attempted: the code may already be used.
  processedDeepLinks.add(key);
  if (processedDeepLinks.size > MAX_PROCESSED_LINKS) {
    const oldest = processedDeepLinks.values().next().value;
    if (oldest) processedDeepLinks.delete(oldest);
  }

  const request = invoke<AccountStateSnapshot>("account_handle_deep_link", { url })
    .finally(() => {
      inFlightDeepLinks.delete(key);
    });
  inFlightDeepLinks.set(key, request);
  return request;
}

async function dispatchDeepLinks(
  urls: string[],
  onSnapshot: (snapshot: AccountStateSnapshot) => void,
  onError: (error: unknown) => void,
): Promise<void> {
  for (const url of urls) {
    try {
      const snapshot = await processDeepLink(url);
      if (snapshot) onSnapshot(snapshot);
    } catch (error) {
      onError(error);
    }
  }
}

/**
 * Registers the runtime listener before reading the cold-start value so a link
 * arriving during initialization cannot fall between the two code paths.
 */
export async function listenForAccountDeepLinks(
  onSnapshot: (snapshot: AccountStateSnapshot) => void,
  onError: (error: unknown) => void,
): Promise<() => void> {
  let unlisten: () => void;
  try {
    unlisten = await onOpenUrl((urls) => {
      void dispatchDeepLinks(urls, onSnapshot, onError);
    });
  } catch {
    throw { code: "callback_listener_failed", stage: "callbackListener" };
  }

  try {
    const currentUrls = await getCurrent();
    if (currentUrls) await dispatchDeepLinks(currentUrls, onSnapshot, onError);
  } catch {
    // Keep the live listener installed even if cold-start lookup fails.
    onError({ code: "callback_read_failed", stage: "callbackRead" });
  }

  return unlisten;
}
