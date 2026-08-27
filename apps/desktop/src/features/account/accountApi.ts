import { invoke } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { AccountState, AccountStateSnapshot } from "./accountTypes";

const processedDeepLinks = new Set<string>();
const inFlightDeepLinks = new Map<string, Promise<AccountStateSnapshot>>();

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

function processDeepLink(url: string): Promise<AccountStateSnapshot | null> {
  if (processedDeepLinks.has(url)) return Promise.resolve(null);

  const existing = inFlightDeepLinks.get(url);
  if (existing) return existing;

  const request = invoke<AccountStateSnapshot>("account_handle_deep_link", { url })
    .then((snapshot) => {
      processedDeepLinks.add(url);
      return snapshot;
    })
    .finally(() => {
      inFlightDeepLinks.delete(url);
    });
  inFlightDeepLinks.set(url, request);
  return request;
}

async function dispatchDeepLinks(
  urls: string[],
  onSnapshot: (snapshot: AccountStateSnapshot) => void,
): Promise<void> {
  for (const url of urls) {
    const snapshot = await processDeepLink(url);
    if (snapshot) onSnapshot(snapshot);
  }
}

/**
 * Registers the runtime listener before reading the cold-start value so a link
 * arriving during initialization cannot fall between the two code paths.
 */
export async function listenForAccountDeepLinks(
  onSnapshot: (snapshot: AccountStateSnapshot) => void,
  onError: () => void,
): Promise<() => void> {
  const unlisten = await onOpenUrl((urls) => {
    void dispatchDeepLinks(urls, onSnapshot).catch(onError);
  });

  try {
    const currentUrls = await getCurrent();
    if (currentUrls) await dispatchDeepLinks(currentUrls, onSnapshot);
  } catch (error) {
    unlisten();
    throw error;
  }

  return unlisten;
}
