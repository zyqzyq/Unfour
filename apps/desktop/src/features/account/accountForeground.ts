import { getCurrentWindow } from "@tauri-apps/api/window";

export type AccountFocusSubscriber = (
  listener: (focused: boolean) => void,
) => Promise<() => void>;

function subscribeToCurrentWindowFocus(
  listener: (focused: boolean) => void,
): Promise<() => void> {
  return getCurrentWindow().onFocusChanged((event) => listener(event.payload));
}

/**
 * Uses the native Tauri focus event as the single foreground signal. Keeping
 * one signal avoids a focus + visibility double refresh when returning from
 * the browser after checkout.
 */
export function listenForAccountForeground(
  onForeground: () => void,
  subscribe: AccountFocusSubscriber = subscribeToCurrentWindowFocus,
): Promise<() => void> {
  return subscribe((focused) => {
    if (focused) onForeground();
  });
}
