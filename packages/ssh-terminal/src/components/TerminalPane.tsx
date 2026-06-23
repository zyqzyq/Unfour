import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
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
import { redactTerminalLog, useTerminalStore } from "../model/terminal-state";

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
  const renderedEventsRef = useRef(0);
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
              });
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
      convertEol: true,
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
  // Render polling-based events (fallback for non-Tauri / mock mode)
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
      renderedSessionIdRef.current = sessionId;
    } else if (events.length < renderedEventsRef.current) {
      // The session's events were cleared/truncated; redraw from scratch.
      terminal.reset();
      renderedEventsRef.current = 0;
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

    // Replay the full backlog when (re)entering a session. Afterwards, under the
    // Tauri runtime live output is painted straight to xterm by the listener
    // below, so skip the diff-write here to avoid duplicating it. Mock mode (no
    // event stream) keeps rendering incremental output through this path.
    if (sessionChanged || !isTauriRuntime()) {
      events.slice(renderedEventsRef.current).forEach((event) => {
        const data =
          event.kind === "input"
            ? `$ ${redactTerminalLog(event.data)}`
            : redactTerminalLog(event.data);
        terminal.write(event.kind === "output" ? data : ensureNewline(data));
      });
    }
    renderedEventsRef.current = events.length;
  }, [events, session]);

  // Live terminal output: write directly to xterm so keystroke echo appears
  // instantly, independent of React state updates and re-renders. Tauri runtime
  // only; the effect above replays history and covers mock mode.
  useEffect(() => {
    const sessionId = session?.sessionId;
    if (!sessionId || !isTauriRuntime()) {
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listen<{ sessionId: string; data?: string }>("ssh://terminal-data", (event) => {
      const payload = event.payload;
      if (payload?.sessionId !== sessionId || !payload.data) {
        return;
      }
      terminalRef.current?.write(redactTerminalLog(payload.data));
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
        } else {
          unlisten = dispose;
        }
      })
      .catch(() => {
        /* mock mode has no Tauri event transport */
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [session?.sessionId]);

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
