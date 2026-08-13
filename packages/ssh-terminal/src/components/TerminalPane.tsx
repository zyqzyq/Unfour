import { useEffect, useRef } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  listSshCommandHistory,
  resizeSshSession,
  sendSshInput,
  type SshSessionEvent,
  type SshSessionSummary,
} from "@unfour/command-client";
import { cn, useFeedbackErrorHandler } from "@unfour/ui";
import { TerminalCommandHistoryController } from "../model/command-history";
import { useTerminalStore } from "../model/terminal-state";
import { sanitizeTerminalWriteChunk } from "../model/terminal-write-sanitizer";
import { TerminalContextMenu } from "./TerminalContextMenu";

export function TerminalPane({
  active,
  className,
  events,
  inputDisabled,
  paintActive = true,
  readOnly,
  session,
}: {
  active?: boolean;
  className?: string;
  events: SshSessionEvent[];
  inputDisabled?: boolean;
  /** When false (Connections hidden under Tasks), skip xterm writes/refreshes. */
  paintActive?: boolean;
  readOnly?: boolean;
  session: SshSessionSummary | null;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);
  const lastSizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const inputQueueRef = useRef(Promise.resolve());
  const lastInputErrorToastAtRef = useRef(0);
  const lastRenderedEventRef = useRef<SshSessionEvent | null>(null);
  const renderedEmptyStateRef = useRef(false);
  const renderedSessionIdRef = useRef<string | null>(null);
  const commandHistoryRef = useRef(new TerminalCommandHistoryController());
  const lastSessionStatusRef = useRef(session?.status);

  const appendTerminalEvents = useTerminalStore((s) => s.appendTerminalEvents);
  const setTerminalSearchAddon = useTerminalStore((s) => s.setTerminalSearchAddon);
  const handleError = useFeedbackErrorHandler();

  // Mutable callback refs – updated in useEffect (not during render).
  const onSendInputRef = useRef<((data: string) => void) | null>(null);
  const onResizeRef = useRef<
    ((sessionId: string, cols: number, rows: number) => void) | null
  >(null);
  const inputDisabledRef = useRef(inputDisabled);
  const readOnlyRef = useRef(readOnly);
  const sessionIdRef = useRef(session?.sessionId ?? null);

  // Keep callback refs in sync with latest store / prop values.
  useEffect(() => {
    const sessionId = session?.sessionId ?? null;
    const workspaceId = session?.workspaceId ?? "";

    onSendInputRef.current =
      sessionId && workspaceId && !readOnly && !inputDisabled
        ? (data: string) => {
            inputQueueRef.current = inputQueueRef.current
              .catch(() => undefined)
              .then(() =>
                sendSshInput({
                  workspaceId,
                  sessionId,
                  data,
                })
                  .then((event) => {
                    if (!isTauriRuntime()) {
                      appendTerminalEvents([event]);
                    }
                  })
                  .catch((error) => {
                    // Input is sent per keystroke, so a toast on every failure
                    // would spam. Surface at most one error per window; the
                    // output stream keeps working regardless.
                    const now = Date.now();
                    if (now - lastInputErrorToastAtRef.current >= INPUT_ERROR_THROTTLE_MS) {
                      lastInputErrorToastAtRef.current = now;
                      handleError(error, { key: "feedback.ssh.inputFailed" });
                    }
                  }),
              );
          }
        : null;

    onResizeRef.current =
      sessionId && workspaceId && !readOnly && !inputDisabled
      ? (sessionId: string, cols: number, rows: number) => {
          resizeSshSession({
            workspaceId,
            sessionId,
            cols,
            rows,
          }).catch((error) => {
            // Resize failures are non-fatal – the terminal just keeps its
            // current size. Log for diagnostics; no user-facing toast.
            console.warn("[ssh-terminal] resize failed", error);
          });
        }
      : null;
  }, [
    appendTerminalEvents,
    handleError,
    inputDisabled,
    readOnly,
    session?.sessionId,
    session?.workspaceId,
  ]);

  useEffect(() => {
    const nextSessionId = session?.sessionId ?? null;
    if (sessionIdRef.current !== nextSessionId) {
      lastSizeRef.current = null;
    }
    inputDisabledRef.current = inputDisabled;
    readOnlyRef.current = readOnly;
    sessionIdRef.current = nextSessionId;
  }, [inputDisabled, readOnly, session?.sessionId]);

  useEffect(() => {
    const controller = commandHistoryRef.current;
    controller.reset();
    const workspaceId = session?.workspaceId;
    const connectionId = session?.connectionId;
    if (!workspaceId || !connectionId || readOnly) return undefined;

    let cancelled = false;
    listSshCommandHistory({ workspaceId, connectionId, limit: 100 })
      .then((entries) => {
        if (!cancelled) controller.setHistory(entries.map((entry) => entry.command));
      })
      .catch((error) => {
        // History recall is additive to the PTY. A failed read must not block
        // terminal input or compete with higher-value connection errors.
        console.warn("[ssh-terminal] command history load failed", error);
      });
    return () => {
      cancelled = true;
    };
  }, [readOnly, session?.connectionId, session?.sessionId, session?.workspaceId]);

  useEffect(() => {
    const status = session?.status;
    const wasReconnecting =
      lastSessionStatusRef.current === "reconnecting" ||
      lastSessionStatusRef.current === "degraded";
    lastSessionStatusRef.current = status;
    if (wasReconnecting && status === "connected") {
      commandHistoryRef.current.resetCurrentLine();
    }
  }, [session?.sessionId, session?.status]);

  // ------------------------------------------------------------------
  // Terminal initialisation
  // ------------------------------------------------------------------

  useEffect(() => {
    if (!hostRef.current) {
      return;
    }

    const styles = getComputedStyle(document.documentElement);
    const token = (name: string) => styles.getPropertyValue(name).trim();
    const terminal = new XTerm({
      // The PTY stream already carries correct CR/LF and cursor-positioning
      // control sequences. `convertEol` would rewrite bare `\n` into `\r\n`,
      // which corrupts the rendering of full-screen apps (vi, less, top) that
      // move the cursor explicitly. Write the bytes through verbatim instead.
      cursorBlink: true,
      fontFamily: "JetBrains Mono, Consolas, ui-monospace, monospace",
      fontSize: 13,
      theme: {
        background: token("--u-color-terminal-bg"),
        cursor: token("--u-color-terminal-cursor"),
        foreground: token("--u-color-terminal-text"),
      },
    });
    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(searchAddon);
    terminal.open(hostRef.current);
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
    searchAddonRef.current = searchAddon;

    // WebView2 can leave xterm paused after a pane was hidden. Disable that
    // observer once for this terminal; `paintActive` already prevents hidden
    // panes from consuming output/render work.
    window.requestAnimationFrame(() => resumeTerminalRendering(terminal));

    const syncFittedSize = () => {
      fitAndSyncTerminalSize(terminal, fitAddon, lastSizeRef, (cols, rows) => {
        const sid = sessionIdRef.current;
        if (sid) {
          onResizeRef.current?.(sid, cols, rows);
        }
      });
    };
    syncFittedSize();

    // ---------------------------------------------------------------
    // Capture keyboard input from xterm
    // ---------------------------------------------------------------
    const dataDisposable = terminal.onData((data: string) => {
      commandHistoryRef.current.accept(data);
      onSendInputRef.current?.(data);
    });

    terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (
        event.type !== "keydown" ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        readOnlyRef.current ||
        inputDisabledRef.current ||
        commandHistoryRef.current.blocksHistoryRecall() ||
        !isNormalTerminalBuffer(terminal)
      ) {
        return true;
      }
      const replacement =
        event.key === "ArrowUp"
          ? commandHistoryRef.current.previous()
          : event.key === "ArrowDown"
            ? commandHistoryRef.current.next()
            : undefined;
      if (replacement === undefined) return true;

      // Readline's Ctrl+U / Ctrl+K pair clears both sides of the current remote
      // cursor without sending Enter. The replacement remains editable.
      onSendInputRef.current?.(`\x15\x0b${replacement}`);
      return false;
    });

    // ---------------------------------------------------------------
    // Detect resize changes from FitAddon
    // ---------------------------------------------------------------
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      const lastSize = lastSizeRef.current;
      if (!lastSize || cols !== lastSize.cols || rows !== lastSize.rows) {
        lastSizeRef.current = { cols, rows };
        const sid = sessionIdRef.current;
        if (sid) {
          onResizeRef.current?.(sid, cols, rows);
        }
      }
    });

    // ---------------------------------------------------------------
    // ResizeObserver for container size changes
    // ---------------------------------------------------------------
    const resizeObserver =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            syncFittedSize();
          });
    resizeObserver?.observe(hostRef.current);

    return () => {
      dataDisposable.dispose();
      resizeDisposable.dispose();
      resizeObserver?.disconnect();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      searchAddonRef.current = null;
      lastSizeRef.current = null;
      lastRenderedEventRef.current = null;
      renderedEmptyStateRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (active) {
      setTerminalSearchAddon(searchAddonRef.current);
      window.requestAnimationFrame(() => terminalRef.current?.focus());
      return () => setTerminalSearchAddon(null);
    }
    return undefined;
  }, [active, setTerminalSearchAddon]);

  // ------------------------------------------------------------------
  // Re-fit on active / session changes
  // ------------------------------------------------------------------

  useEffect(() => {
    const fitAddon = fitAddonRef.current;
    const terminal = terminalRef.current;
    if (fitAddon && terminal && paintActive) {
      window.requestAnimationFrame(() => {
        resumeTerminalRendering(terminal);
        fitAndSyncTerminalSize(terminal, fitAddon, lastSizeRef, (cols, rows) => {
          const sid = sessionIdRef.current;
          if (sid) {
            onResizeRef.current?.(sid, cols, rows);
          }
        });
        terminal.refresh(0, terminal.rows - 1);
        if (active) {
          terminal.focus();
        }
      });
    }
  }, [active, paintActive, readOnly, session?.cols, session?.rows, session?.sessionId]);

  // ------------------------------------------------------------------
  // Paint terminal output from the store (single source of truth)
  // ------------------------------------------------------------------

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal || !paintActive) {
      return;
    }

    const sessionId = session?.sessionId ?? null;
    const sessionChanged = renderedSessionIdRef.current !== sessionId;
    if (sessionChanged) {
      terminal.reset();
      lastRenderedEventRef.current = null;
      renderedEmptyStateRef.current = false;
      renderedSessionIdRef.current = sessionId;
    }

    if (events.length === 0) {
      if (lastRenderedEventRef.current) {
        // The session was explicitly cleared. Pruning a live stream never
        // removes its newest rendered event, so an empty list is a real reset.
        lastRenderedEventRef.current = null;
        renderedEmptyStateRef.current = false;
      }
      if (renderedEmptyStateRef.current) {
        return;
      }
      terminal.reset();
      terminal.write(
        session
          ? session.status === "connected"
            ? `Connected to ${session.username}@${session.host}. Waiting for output.\r\n`
            : `Session ${session.username}@${session.host} is disconnected.\r\n`
          : "Select a connection and start a session.\r\n",
      );
      renderedEmptyStateRef.current = true;
      return;
    }

    if (renderedEmptyStateRef.current) {
      terminal.reset();
      renderedEmptyStateRef.current = false;
    }

    // Replay the full backlog when (re)entering a session, then paint each
    // incremental diff as new events land. Live output reaches the store via the
    // single global listener in TerminalPage (coalesced ~once per frame), so
    // this one store-driven writer covers both Tauri and mock mode. A separate
    // per-pane live listener used to paint Tauri output directly, but it
    // registered asynchronously after mount and therefore missed the burst of
    // early output (login banner, first prompt) — which then only surfaced after
    // a tab switch forced a full replay. Painting solely from the store removes
    // that race without adding re-renders (the global listener drives them
    // either way).
    // Write PTY output bytes to xterm verbatim. Redaction of persisted history
    // happens in the backend (terminal_history) and the exported log view
    // (TerminalLogPanel); applying line-based redaction to the live stream here
    // would mangle the cursor-addressing escape sequences that full-screen apps
    // emit, breaking their rendering.
    const sanitizeWrite = (chunk: string) => {
      const sanitized = sanitizeTerminalWriteChunk(chunk);
      if (sanitized.removedSequences.length) {
        console.warn("[ssh-terminal] filtered xterm request-mode sequence", {
          sessionId: sessionIdRef.current,
          removedSequences: sanitized.removedSequences,
        });
      }
      if (sanitized.value.length === 0) {
        return "";
      }
      return sanitized.value;
    };

    const lastRenderedIndex = lastRenderedEventRef.current
      ? events.indexOf(lastRenderedEventRef.current)
      : -1;
    // Cursor can disappear while Connections is hidden under Tasks and the
    // bounded store prunes the paused tip — reset and replay the retained tail.
    if (lastRenderedIndex < 0 && lastRenderedEventRef.current) {
      terminal.reset();
      renderedEmptyStateRef.current = false;
    }
    const startIndex = lastRenderedIndex < 0 ? 0 : lastRenderedIndex + 1;
    // React already coalesces the live stream to roughly one update per frame.
    // Give xterm one combined write for that update instead of one write and
    // full viewport refresh per event. xterm schedules its own incremental
    // renderer, avoiding an unbounded WebView2 raster/resource queue.
    const writeBatch = events
      .slice(startIndex)
      .map((event) => {
        if (event.kind === "output") {
          commandHistoryRef.current.observeOutput(event.data);
        }
        const data = event.kind === "input" ? `$ ${event.data}` : event.data;
        return sanitizeWrite(event.kind === "output" ? data : ensureNewline(data));
      })
      .join("");
    if (writeBatch) {
      terminal.write(writeBatch);
    }
    lastRenderedEventRef.current = events[events.length - 1] ?? null;
  }, [events, paintActive, session]);

  return (
    <TerminalContextMenu
      canPaste={Boolean(session?.sessionId && !readOnly && !inputDisabled)}
      terminalRef={terminalRef}
    >
      <div
        className={cn(
          "min-h-0 flex-1 overflow-hidden bg-[var(--u-color-terminal-bg)] p-2",
          className,
        )}
        onClick={() => terminalRef.current?.focus()}
      >
        <div className="h-full min-h-0 w-full overflow-hidden" ref={hostRef} />
      </div>
    </TerminalContextMenu>
  );
}

// xterm v6 gates ALL rendering on a private `RenderService._isPaused` flag that
// is driven by an IntersectionObserver (it pauses rendering when the terminal is
// off-screen to save CPU). In the production WebView2 build that observer
// reports the visible terminal as not intersecting and never corrects itself, so
// `_isPaused` stays `true` and every write/refresh is silently dropped (the
// terminal renders once, then freezes). xterm exposes no public override, so we
// reach into the internal service once when the terminal opens or becomes
// visible. Hidden output is already paused by the public `paintActive` gate.
// Guarded so an xterm internals change degrades gracefully instead of crashing.
function resumeTerminalRendering(terminal: XTerm) {
  try {
    const renderService = (
      terminal as unknown as {
        _core?: {
          _renderService?: {
            _isPaused?: boolean;
            _observerDisposable?: { clear?: () => void };
          };
        };
      }
    )._core?._renderService;
    if (!renderService) {
      return;
    }
    // Stop the observer so it cannot re-pause us between writes.
    renderService._observerDisposable?.clear?.();
    renderService._isPaused = false;
  } catch {
    // Best-effort renderer kick; ignore if xterm internals changed.
  }
}

function safeFit(fitAddon: FitAddon) {
  try {
    fitAddon.fit();
  } catch {
    // The pane may be hidden during a shell resize. ResizeObserver retries once visible.
  }
}

function fitAndSyncTerminalSize(
  terminal: XTerm,
  fitAddon: FitAddon,
  lastSizeRef: { current: { cols: number; rows: number } | null },
  notifyResize: (cols: number, rows: number) => void,
) {
  safeFit(fitAddon);
  const nextSize = { cols: terminal.cols, rows: terminal.rows };
  const lastSize = lastSizeRef.current;
  if (!lastSize || nextSize.cols !== lastSize.cols || nextSize.rows !== lastSize.rows) {
    lastSizeRef.current = nextSize;
    notifyResize(nextSize.cols, nextSize.rows);
  }
}

function ensureNewline(value: string) {
  return value.endsWith("\r\n") || value.endsWith("\n") ? value : `${value}\r\n`;
}

function isNormalTerminalBuffer(terminal: XTerm) {
  const type = (
    terminal as XTerm & {
      buffer?: { active?: { type?: "normal" | "alternate" } };
    }
  ).buffer?.active?.type;
  return type === undefined || type === "normal";
}

// Cap how often a failed-keystroke error can surface so a disconnected session
// doesn't flood the user with a toast on every character typed.
const INPUT_ERROR_THROTTLE_MS = 4000;

function isTauriRuntime() {
  return (
    typeof window !== "undefined" &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}

