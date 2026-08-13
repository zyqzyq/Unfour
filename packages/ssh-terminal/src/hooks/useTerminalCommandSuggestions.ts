import { useCallback, useRef, useState } from "react";
import type { Terminal as XTerm } from "@xterm/xterm";
import type { TerminalCommandHistoryController } from "../model/command-history";

export type TerminalSuggestionState = {
  anchor: { top?: number; bottom?: number };
  items: string[];
  selected: number;
};

const MAX_SUGGESTION_ITEMS = 6;

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
    applySuggestions(null);
  }, [applySuggestions]);

  const suppressSuggestions = useCallback(() => {
    // Esc dismissed suggestions for this line. refreshSuggestions re-arms
    // once the line is submitted or cleared, mirroring how mature SSH
    // clients re-trigger autocomplete after the next executed command.
    suppressedRef.current = true;
    applySuggestions(null);
  }, [applySuggestions]);

  const refreshSuggestions = useCallback(() => {
    const terminal = terminalRef.current;
    const controller = controllerRef.current;
    if (!terminal || !onSendInputRef.current || !isNormalTerminalBuffer(terminal)) {
      applySuggestions(null);
      return;
    }
    if (suppressedRef.current) {
      if (controller.currentLine().length > 0) {
        applySuggestions(null);
        return;
      }
      suppressedRef.current = false;
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
    const rowHeight = hostHeight / rows;
    const cursorRow = cursorViewportRow(terminal);
    // Open below the cursor in the upper half of the viewport, above it in the
    // lower half (where the prompt usually sits).
    const anchor =
      cursorRow < rows / 2
        ? { top: Math.round((cursorRow + 1) * rowHeight) }
        : { bottom: Math.round(hostHeight - cursorRow * rowHeight) };
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
        applySuggestions(null);
        return;
      }
      controller.accept(payload);
      send(payload);
      refreshSuggestions();
      terminalRef.current?.focus();
    },
    [applySuggestions, controllerRef, onSendInputRef, refreshSuggestions, terminalRef],
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
