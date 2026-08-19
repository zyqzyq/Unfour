import { useCallback, useRef, useState } from "react";
import type { Terminal as XTerm } from "@xterm/xterm";
import type { TerminalCommandHistoryController } from "../model/command-history";

export type TerminalSuggestionState = {
  anchor: { top?: number; bottom?: number };
  items: string[];
  selected: number;
};

const MAX_SUGGESTION_ITEMS = 6;
const SUGGESTION_ITEM_HEIGHT_PX = 26;
const SUGGESTION_LIST_PADDING_PX = 4;
const SUGGESTION_HINT_HEIGHT_PX = 22;
const SUGGESTION_GAP_PX = 6;

/**
 * Suggestion-popup state machine for TerminalPane. Owns when the popup is
 * visible, which entry is selected, where it anchors, and how an accepted
 * command is turned into PTY input. All collaborators arrive as refs so the
 * returned callbacks stay referentially stable for xterm's one-time handlers.
 */
export function useTerminalCommandSuggestions({
  controllerRef,
  hostRef,
  onSendInputRef,
  terminalRef,
}: {
  controllerRef: { current: TerminalCommandHistoryController };
  hostRef: { current: HTMLDivElement | null };
  onSendInputRef: { current: ((data: string) => void) | null };
  terminalRef: { current: XTerm | null };
}) {
  const suggestionsRef = useRef<TerminalSuggestionState | null>(null);
  const suppressedRef = useRef(false);
  const suppressedLineRef = useRef<string | null>(null);
  const [suggestions, setSuggestions] = useState<TerminalSuggestionState | null>(null);

  const applySuggestions = useCallback((next: TerminalSuggestionState | null) => {
    const previous = suggestionsRef.current;
    if (!next && !previous) return;
    if (
      next &&
      previous &&
      next.selected === previous.selected &&
      next.anchor.top === previous.anchor.top &&
      next.anchor.bottom === previous.anchor.bottom &&
      sameSuggestionItems(next.items, previous.items)
    ) {
      return;
    }
    suggestionsRef.current = next;
    setSuggestions(next);
  }, []);

  const resetSuggestions = useCallback(() => {
    suppressedRef.current = false;
    suppressedLineRef.current = null;
    applySuggestions(null);
  }, [applySuggestions]);

  const suppressSuggestions = useCallback(() => {
    // Esc dismissed suggestions for this line. refreshSuggestions re-arms
    // once the line is submitted or cleared, mirroring how mature SSH
    // clients re-trigger autocomplete after the next executed command.
    suppressedRef.current = true;
    suppressedLineRef.current = null;
    applySuggestions(null);
  }, [applySuggestions]);

  const refreshSuggestions = useCallback(() => {
    const terminal = terminalRef.current;
    const controller = controllerRef.current;
    if (!terminal || !onSendInputRef.current || !isNormalTerminalBuffer(terminal)) {
      applySuggestions(null);
      return;
    }
    if (
      keepSuggestionsSuppressed(
        controller,
        suppressedRef,
        suppressedLineRef,
        applySuggestions,
      )
    ) {
      return;
    }
    const lineState = controller.lineState();
    if (
      controller.promptContext() !== "shell" ||
      !lineState.reliable ||
      !lineState.cursorAtEnd
    ) {
      applySuggestions(null);
      return;
    }
    const items = controller.suggest(MAX_SUGGESTION_ITEMS);
    if (items.length === 0) {
      applySuggestions(null);
      return;
    }
    const host = hostRef.current;
    const rows = Math.max(terminal.rows, 1);
    const hostHeight = host?.clientHeight ?? 0;
    const cursorRow = Math.min(Math.max(cursorViewportRow(terminal), 0), rows - 1);
    const anchor = getSuggestionAnchor(hostHeight, rows, cursorRow, items.length);
    const previous = suggestionsRef.current;
    applySuggestions({
      anchor,
      items,
      selected:
        previous && sameSuggestionItems(previous.items, items) ? previous.selected : 0,
    });
  }, [applySuggestions, controllerRef, hostRef, onSendInputRef, terminalRef]);

  const acceptSuggestion = useCallback(
    (command: string) => {
      const send = onSendInputRef.current;
      if (!send) return;
      const controller = controllerRef.current;
      const line = controller.currentLine();
      // Prefix completions append only the missing suffix. Substring matches
      // clear the typed line with plain backspaces first — unlike readline
      // control shortcuts, backspace works in every remote line editor.
      const payload = command.startsWith(line)
        ? command.slice(line.length)
        : `${"\x7f".repeat(Array.from(line).length)}${command}`;
      if (payload.length === 0) {
        suppressedRef.current = true;
        suppressedLineRef.current = controller.currentLine();
        applySuggestions(null);
        terminalRef.current?.focus();
        return;
      }
      controller.accept(payload);
      send(payload);
      // Do not immediately reopen the popup for the just-inserted exact
      // command. Re-arm as soon as the user edits that line again.
      suppressedRef.current = true;
      suppressedLineRef.current = controller.currentLine();
      applySuggestions(null);
      terminalRef.current?.focus();
    },
    [applySuggestions, controllerRef, onSendInputRef, terminalRef],
  );

  return {
    acceptSuggestion,
    applySuggestions,
    refreshSuggestions,
    resetSuggestions,
    suggestions,
    suggestionsRef,
    suppressSuggestions,
  };
}

function sameSuggestionItems(a: string[], b: string[]) {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

function keepSuggestionsSuppressed(
  controller: TerminalCommandHistoryController,
  suppressedRef: { current: boolean },
  suppressedLineRef: { current: string | null },
  applySuggestions: (next: TerminalSuggestionState | null) => void,
) {
  if (!suppressedRef.current) return false;
  if (suppressedLineRef.current !== null) {
    if (controller.currentLine() === suppressedLineRef.current) {
      applySuggestions(null);
      return true;
    }
    suppressedRef.current = false;
    suppressedLineRef.current = null;
    return false;
  }
  if (controller.currentLine().length > 0) {
    applySuggestions(null);
    return true;
  }
  suppressedRef.current = false;
  return false;
}

function getSuggestionAnchor(
  hostHeight: number,
  rows: number,
  cursorRow: number,
  itemCount: number,
): { top?: number; bottom?: number } {
  const rowHeight = hostHeight / rows;
  const popupHeight =
    itemCount * SUGGESTION_ITEM_HEIGHT_PX +
    SUGGESTION_LIST_PADDING_PX +
    SUGGESTION_HINT_HEIGHT_PX;
  const cursorTop = cursorRow * rowHeight;
  const cursorBottom = (cursorRow + 1) * rowHeight;
  const spaceAbove = cursorTop;
  const spaceBelow = Math.max(0, hostHeight - cursorBottom);
  const canFitAbove = spaceAbove >= popupHeight + SUGGESTION_GAP_PX;
  const canFitBelow = spaceBelow >= popupHeight + SUGGESTION_GAP_PX;
  // Keep the popup outside the active terminal row. Prefer the side with
  // enough room; when neither side can fit the full list, use the side with
  // more space so the current command is never covered by the popup box.
  if (canFitAbove || (!canFitBelow && spaceAbove >= spaceBelow)) {
    return {
      bottom: Math.round(hostHeight - cursorTop + SUGGESTION_GAP_PX),
    };
  }
  return { top: Math.round(cursorBottom + SUGGESTION_GAP_PX) };
}

function cursorViewportRow(terminal: XTerm) {
  const cursorY = (
    terminal as XTerm & { buffer?: { active?: { cursorY?: number } } }
  ).buffer?.active?.cursorY;
  return typeof cursorY === "number" ? cursorY : Math.max(0, terminal.rows - 1);
}

function isNormalTerminalBuffer(terminal: XTerm) {
  const type = (
    terminal as XTerm & {
      buffer?: { active?: { type?: "normal" | "alternate" } };
    }
  ).buffer?.active?.type;
  return type === undefined || type === "normal";
}
