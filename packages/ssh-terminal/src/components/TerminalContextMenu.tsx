import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import type { Terminal as XTerm } from "@xterm/xterm";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  useFeedbackErrorHandler,
  useI18n,
  usePlatform,
} from "@unfour/ui";
import { readTerminalClipboard } from "../model/terminal-clipboard";

export function TerminalContextMenu({
  canPaste,
  children,
  pasteTarget,
  terminalRef,
}: {
  canPaste: boolean;
  children: ReactNode;
  pasteTarget: string | null;
  terminalRef: RefObject<XTerm | null>;
}) {
  const [hasSelection, setHasSelection] = useState(false);
  const [clipboardPending, setClipboardPending] = useState(false);
  const clipboardPendingRef = useRef(false);
  const targetEpochRef = useRef(0);
  const handleError = useFeedbackErrorHandler();
  const { t } = useI18n();
  const platform = usePlatform();
  const copyShortcut = platform === "macos" ? "⌘C" : "Ctrl+Shift+C";
  const pasteShortcut = platform === "macos" ? "⌘V" : "Ctrl+V";
  const hasClipboard = typeof navigator !== "undefined" && Boolean(navigator.clipboard);

  useEffect(() => {
    // A permission prompt may outlive this tab, session, or writable state.
    // Never deliver its result to the target that happens to be current later.
    targetEpochRef.current += 1;
    return () => { targetEpochRef.current += 1; };
  }, [canPaste, pasteTarget]);

  function copySelection() {
    const selection = terminalRef.current?.getSelection() ?? "";
    if (!selection || !navigator.clipboard || clipboardPendingRef.current) {
      return;
    }
    clipboardPendingRef.current = true;
    setClipboardPending(true);
    const terminal = terminalRef.current;
    const epoch = targetEpochRef.current;
    void navigator.clipboard
      .writeText(selection)
      .catch((error) => handleError(error, { key: "feedback.ssh.clipboardWriteFailed" }))
      .finally(() => finishClipboardAction(terminal, epoch));
  }

  function pasteSelection() {
    const selection = terminalRef.current?.getSelection() ?? "";
    if (!canPaste || !selection) {
      return;
    }
    terminalRef.current?.paste(selection);
  }

  function pasteClipboard() {
    if (!canPaste || !pasteTarget || !navigator.clipboard || clipboardPendingRef.current) {
      return;
    }
    const terminal = terminalRef.current;
    const epoch = targetEpochRef.current;
    const isCurrent = () => terminal !== null && terminalRef.current === terminal && targetEpochRef.current === epoch;
    clipboardPendingRef.current = true;
    setClipboardPending(true);
    void readTerminalClipboard(navigator.clipboard, isCurrent, () => terminal?.focus())
      .then((text) => {
        if (text && isCurrent()) {
          terminal?.paste(text);
        }
      })
      .catch((error) => {
        if (isCurrent()) handleError(error, { key: "feedback.ssh.clipboardReadFailed" });
      })
      .finally(() => finishClipboardAction(terminal, epoch));
  }

  function finishClipboardAction(terminal: XTerm | null, epoch: number) {
    clipboardPendingRef.current = false;
    setClipboardPending(false);
    if (targetEpochRef.current === epoch && terminalRef.current === terminal && document.hasFocus()) {
      terminal?.focus();
    }
  }

  return (
    <ContextMenu
      onOpenChange={(open) => {
        if (open) {
          setHasSelection(Boolean(terminalRef.current?.hasSelection()));
        }
      }}
    >
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent onCloseAutoFocus={(event) => {
        event.preventDefault();
        // The permission popup owns focus until the clipboard operation settles.
        if (!clipboardPendingRef.current) terminalRef.current?.focus();
      }}>
        <ContextMenuItem
          disabled={!hasSelection || !hasClipboard || clipboardPending}
          onSelect={copySelection}
          shortcut={copyShortcut}
        >
          {t("ssh.actions.copySelection")}
        </ContextMenuItem>
        <ContextMenuItem
          disabled={!canPaste || !hasSelection}
          onSelect={pasteSelection}
        >
          {t("ssh.actions.pasteSelection")}
        </ContextMenuItem>
        <ContextMenuItem
          disabled={!canPaste || !hasClipboard || clipboardPending}
          onSelect={pasteClipboard}
          shortcut={pasteShortcut}
        >
          {t("ssh.actions.pasteClipboard")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => terminalRef.current?.selectAll()}>
          {t("ssh.actions.selectAll")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
