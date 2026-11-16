/**
 * Chromium/WebView2 can reject the first read while returning focus from its
 * permission prompt. Retry only after an explicit grant, never after denial.
 * The caller guards the original paste target throughout the asynchronous read.
 */
export async function readTerminalClipboard(
  clipboard: Pick<Clipboard, "readText">,
  isCurrent: () => boolean,
  restoreFocus: () => void,
): Promise<string> {
  try {
    return await clipboard.readText();
  } catch (error) {
    if (!isCurrent() || !(error instanceof DOMException) || error.name !== "NotAllowedError") {
      throw error;
    }
    let permission: PermissionStatus;
    try {
      // Chromium exposes clipboard-read; it is not in TypeScript's cross-browser union.
      permission = await navigator.permissions.query({ name: "clipboard-read" as PermissionName });
    } catch {
      throw error;
    }
    if (permission.state !== "granted" || !isCurrent()) throw error;
    restoreFocus();
    if (!await waitForDocumentFocus()) throw error;
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    if (!isCurrent() || !document.hasFocus()) throw error;
    return clipboard.readText();
  }
}

function waitForDocumentFocus(): Promise<boolean> {
  if (document.hasFocus()) return Promise.resolve(true);
  return new Promise((resolve) => {
    const finish = (focused: boolean) => {
      window.clearTimeout(timeout);
      window.removeEventListener("focus", onFocus);
      resolve(focused);
    };
    const onFocus = () => {
      if (document.hasFocus()) finish(true);
    };
    // Allow the permission dialog's asynchronous close/focus handoff to finish,
    // without leaving a paste pending indefinitely if the user leaves the app.
    const timeout = window.setTimeout(() => finish(false), 1000);
    window.addEventListener("focus", onFocus);
  });
}
