import { useEffect, useRef } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  resizeSshSession,
  sendSshInput,
  type SshSessionEvent,
  type SshSessionSummary,
} from "@unfour/command-client";
import { cn } from "@unfour/ui";
import { useTerminalStore } from "../model/terminal-state";
import { sanitizeTerminalWriteChunk } from "../model/terminal-write-sanitizer";

export function TerminalPane({
  active,
  className,
  events,
  inputDisabled,
  readOnly,
  session,
}: {
  active?: boolean;
  className?: string;
  events: SshSessionEvent[];
  inputDisabled?: boolean;
  readOnly?: boolean;
  session: SshSessionSummary | null;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);
  const lastSizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const inputQueueRef = useRef(Promise.resolve());
  const renderedEventsRef = useRef(0);
  const renderedEventDataLengthsRef = useRef<number[]>([]);
  const renderedSessionIdRef = useRef<string | null>(null);

  const appendTerminalEvents = useTerminalStore((s) => s.appendTerminalEvents);
  const setTerminalSearchAddon = useTerminalStore((s) => s.setTerminalSearchAddon);

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
                  .catch(() => {
                    /* swallow – output stream still works */
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
          }).catch(() => {
            /* resize failures are non-fatal */
          });
        }
      : null;
  }, [appendTerminalEvents, inputDisabled, readOnly, session?.sessionId, session?.workspaceId]);

  useEffect(() => {
    const nextSessionId = session?.sessionId ?? null;
    if (sessionIdRef.current !== nextSessionId) {
      lastSizeRef.current = null;
    }
    inputDisabledRef.current = inputDisabled;
    readOnlyRef.current = readOnly;
    sessionIdRef.current = nextSessionId;
  }, [inputDisabled, readOnly, session?.sessionId]);

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

    // The render-pause observer is registered asynchronously after open(); clear
    // it on the next frame so live output is never silently dropped (see
    // resumeTerminalRendering). The per-write call covers any later re-pause.
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
      onSendInputRef.current?.(data);
    });
    terminal.attachCustomKeyEventHandler((event) => {
      const key = event.key.toLowerCase();
      const modified = event.ctrlKey || event.metaKey;
      if (!modified) {
        return true;
      }

      if (key === "c" && terminal.hasSelection()) {
        void navigator.clipboard?.writeText(terminal.getSelection());
        return false;
      }

      if (
        key === "v" &&
        !readOnlyRef.current &&
        !inputDisabledRef.current &&
        sessionIdRef.current
      ) {
        void navigator.clipboard?.readText().then((text) => {
          if (text) {
            onSendInputRef.current?.(text);
          }
        });
        return false;
      }

      return true;
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
      renderedEventsRef.current = 0;
      renderedEventDataLengthsRef.current = [];
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
    if (fitAddon && terminal) {
      window.requestAnimationFrame(() => {
        fitAndSyncTerminalSize(terminal, fitAddon, lastSizeRef, (cols, rows) => {
          const sid = sessionIdRef.current;
          if (sid) {
            onResizeRef.current?.(sid, cols, rows);
          }
        });
        if (active) {
          terminal.focus();
        }
      });
    }
  }, [active, readOnly, session?.cols, session?.rows, session?.sessionId]);

  // ------------------------------------------------------------------
  // Paint terminal output from the store (single source of truth)
  // ------------------------------------------------------------------

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    const sessionId = session?.sessionId ?? null;
    const sessionChanged = renderedSessionIdRef.current !== sessionId;
    if (sessionChanged) {
      terminal.reset();
      renderedEventsRef.current = 0;
      renderedEventDataLengthsRef.current = [];
      renderedSessionIdRef.current = sessionId;
    } else if (events.length < renderedEventsRef.current) {
      // The session's events were cleared/truncated; redraw from scratch.
      terminal.reset();
      renderedEventsRef.current = 0;
      renderedEventDataLengthsRef.current = [];
    }

    if (events.length === 0 && renderedEventsRef.current === 0) {
      terminal.write(
        session
          ? session.status === "connected"
            ? `Connected to ${session.username}@${session.host}. Waiting for output.\r\n`
            : `Session ${session.username}@${session.host} is disconnected.\r\n`
          : "Select a connection and start a session.\r\n",
      );
      return;
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
    // After the written bytes are parsed into the buffer, force a
    // viewport repaint. The production WebView2 build does not repaint on
    // incremental writes on its own (the initial render works, later writes do
    // not). The callback fires post-parse, so this both confirms parsing and
    // forces the frame.
    const writeToTerminal = (chunk: string) => {
      const sanitized = sanitizeTerminalWriteChunk(chunk);
      if (sanitized.removedSequences.length) {
        console.warn("[ssh-terminal] filtered xterm request-mode sequence", {
          sessionId: sessionIdRef.current,
          removedSequences: sanitized.removedSequences,
        });
      }
      if (sanitized.value.length === 0) {
        return;
      }
      terminal.write(sanitized.value, () => {
        resumeTerminalRendering(terminal);
        terminal.refresh(0, terminal.rows - 1);
      });
    };

    events.slice(0, renderedEventsRef.current).forEach((event, index) => {
      const renderedLength = renderedEventDataLengthsRef.current[index] ?? 0;
      if (event.kind === "output" && event.data.length > renderedLength) {
        writeToTerminal(event.data.slice(renderedLength));
        renderedEventDataLengthsRef.current[index] = event.data.length;
      }
    });

    events.slice(renderedEventsRef.current).forEach((event, index) => {
      const eventIndex = renderedEventsRef.current + index;
      const data = event.kind === "input" ? `$ ${event.data}` : event.data;
      writeToTerminal(event.kind === "output" ? data : ensureNewline(data));
      renderedEventDataLengthsRef.current[eventIndex] = event.data.length;
    });
    renderedEventsRef.current = events.length;
    renderedEventDataLengthsRef.current.length = events.length;
  }, [events, session]);

  return (
    <div
      className={cn(
        "min-h-0 flex-1 overflow-hidden bg-[var(--u-color-terminal-bg)] p-2",
        className,
      )}
      onClick={() => terminalRef.current?.focus()}
    >
      <div className="h-full min-h-0 w-full overflow-hidden" ref={hostRef} />
    </div>
  );
}

// xterm v6 gates ALL rendering on a private `RenderService._isPaused` flag that
// is driven by an IntersectionObserver (it pauses rendering when the terminal is
// off-screen to save CPU). In the production WebView2 build that observer
// reports the visible terminal as not intersecting and never corrects itself, so
// `_isPaused` stays `true` and every write/refresh is silently dropped (the
// terminal renders once, then freezes). xterm exposes no public override, so we
// reach into the internal service to clear the stuck flag and disconnect the
// misfiring observer (disabling the off-screen optimization for this terminal,
// which is exactly the broken behaviour). Guarded so an xterm internals change
// degrades gracefully instead of crashing the pane.
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

function isTauriRuntime() {
  return (
    typeof window !== "undefined" &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}

