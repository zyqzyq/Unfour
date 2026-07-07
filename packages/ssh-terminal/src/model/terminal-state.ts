import { create } from "zustand";
import type { SshSessionEvent, SshSessionSummary } from "@unfour/command-client";
import type { TerminalSplitMode } from "./types";

type SearchAddonLike = {
  findNext: (term: string) => boolean;
  findPrevious: (term: string) => boolean;
  clearDecorations: () => void;
  dispose: () => void;
};

// The slice of terminal UI state that is scoped to a single workspace. The
// search addon is intentionally NOT part of the slice: it is a handle to the
// live xterm instance owned by the mounted `TerminalPage`, so it stays global
// and is never partitioned or cleared on workspace switch.
type TerminalSlice = {
  activeSessionId: string | null;
  dismissedSessionIds: string[];
  exportedLog: string | null;
  frontendFailedSessions: Record<string, SshSessionSummary>;
  searchOpen: boolean;
  searchQuery: string;
  splitMode: TerminalSplitMode;
  terminalEvents: SshSessionEvent[];
  terminalInput: string;
};

type TerminalStore = {
  // Per-workspace archive. `activateWorkspace` swaps the flat fields below to
  // the slice for the newly active workspace instead of clearing them, so each
  // workspace keeps its own terminal buffer/sessions/input across switches.
  byWorkspace: Record<string, TerminalSlice>;
  activeSessionId: string | null;
  dismissedSessionIds: string[];
  exportedLog: string | null;
  frontendFailedSessions: Record<string, SshSessionSummary>;
  searchOpen: boolean;
  searchQuery: string;
  splitMode: TerminalSplitMode;
  terminalEvents: SshSessionEvent[];
  terminalInput: string;
  terminalSearchAddon: SearchAddonLike | null;
  workspaceId: string | null;
  activateWorkspace: (workspaceId: string) => void;
  addFrontendFailedSession: (session: SshSessionSummary) => void;
  appendTerminalEvents: (events: SshSessionEvent[]) => void;
  clearTerminalSessionEvents: (sessionId: string | null) => void;
  dismissSession: (sessionId: string) => void;
  hydrateTerminalSession: (sessionId: string, events: SshSessionEvent[]) => void;
  resetTerminalEvents: () => void;
  setActiveSessionId: (sessionId: string | null) => void;
  setExportedLog: (content: string | null) => void;
  setSearchOpen: (open: boolean) => void;
  setSearchQuery: (query: string) => void;
  setSplitMode: (mode: TerminalSplitMode) => void;
  setTerminalSearchAddon: (addon: SearchAddonLike | null) => void;
  startTerminalSession: (sessionId: string, events: SshSessionEvent[]) => void;
  setTerminalEvents: (events: SshSessionEvent[]) => void;
  setTerminalInput: (input: string) => void;
};

export function defaultTerminalInput() {
  return "";
}

function createDefaultSlice(): TerminalSlice {
  return {
    activeSessionId: null,
    dismissedSessionIds: [],
    exportedLog: null,
    frontendFailedSessions: {},
    searchOpen: false,
    searchQuery: "",
    splitMode: "single",
    terminalEvents: [],
    terminalInput: defaultTerminalInput(),
  };
}

function sliceFromFlat(state: TerminalStore): TerminalSlice {
  return {
    activeSessionId: state.activeSessionId,
    dismissedSessionIds: state.dismissedSessionIds,
    exportedLog: state.exportedLog,
    frontendFailedSessions: state.frontendFailedSessions,
    searchOpen: state.searchOpen,
    searchQuery: state.searchQuery,
    splitMode: state.splitMode,
    terminalEvents: state.terminalEvents,
    terminalInput: state.terminalInput,
  };
}

function flatFromSlice(slice: TerminalSlice) {
  return {
    activeSessionId: slice.activeSessionId,
    dismissedSessionIds: slice.dismissedSessionIds,
    exportedLog: slice.exportedLog,
    frontendFailedSessions: slice.frontendFailedSessions,
    searchOpen: slice.searchOpen,
    searchQuery: slice.searchQuery,
    splitMode: slice.splitMode,
    terminalEvents: slice.terminalEvents,
    terminalInput: slice.terminalInput,
  };
}

export const useTerminalStore = create<TerminalStore>((set) => ({
  byWorkspace: {},
  activeSessionId: null,
  dismissedSessionIds: [],
  exportedLog: null,
  frontendFailedSessions: {},
  searchOpen: false,
  searchQuery: "",
  splitMode: "single",
  terminalEvents: [],
  terminalInput: defaultTerminalInput(),
  terminalSearchAddon: null,
  workspaceId: null,
  activateWorkspace: (workspaceId) =>
    set((state) => {
      if (state.workspaceId === workspaceId) {
        return state;
      }
      // Archive the current flat slice under the previously active workspace
      // (if any), then load the target workspace's slice. This preserves each
      // workspace's terminal buffer/sessions/input across switches instead of
      // clearing them (the old behavior caused a flicker + selection reset and
      // required an async backend refill).
      const nextByWorkspace =
        state.workspaceId !== null
          ? { ...state.byWorkspace, [state.workspaceId]: sliceFromFlat(state) }
          : state.byWorkspace;
      const nextSlice =
        workspaceId !== null
          ? nextByWorkspace[workspaceId] ?? createDefaultSlice()
          : createDefaultSlice();
      return {
        ...flatFromSlice(nextSlice),
        byWorkspace: nextByWorkspace,
        // The search addon belongs to the live xterm instance and must survive
        // workspace switches untouched.
        terminalSearchAddon: state.terminalSearchAddon,
        workspaceId,
      };
    }),
  addFrontendFailedSession: (session) =>
    set((state) => {
      // Remove any previous failed session for the same connectionId so only
      // one stale tab per connection accumulates.
      const next = { ...state.frontendFailedSessions };
      for (const [id, existing] of Object.entries(next)) {
        if (existing.connectionId === session.connectionId) {
          delete next[id];
        }
      }
      next[session.sessionId] = session;
      return { frontendFailedSessions: next };
    }),
  appendTerminalEvents: (events) =>
    set((state) => ({
      terminalEvents: appendCoalescedTerminalEvents(state.terminalEvents, events),
    })),
  clearTerminalSessionEvents: (sessionId) =>
    set((state) => ({
      exportedLog: null,
      terminalEvents: sessionId
        ? state.terminalEvents.filter((event) => event.sessionId !== sessionId)
        : [],
    })),
  dismissSession: (sessionId) =>
    set((state) => {
      // The backend keeps closed sessions in its list as history, so a closed
      // tab would otherwise reappear on the next poll. Track dismissed ids and
      // filter them out of the visible tab strip.
      const nextFailed = { ...state.frontendFailedSessions };
      delete nextFailed[sessionId];
      return {
        activeSessionId: state.activeSessionId === sessionId ? null : state.activeSessionId,
        dismissedSessionIds: state.dismissedSessionIds.includes(sessionId)
          ? state.dismissedSessionIds
          : [...state.dismissedSessionIds, sessionId],
        frontendFailedSessions: nextFailed,
        terminalEvents: state.terminalEvents.filter((event) => event.sessionId !== sessionId),
      };
    }),
  hydrateTerminalSession: (sessionId, events) =>
    set((state) => {
      if (state.terminalEvents.some((event) => event.sessionId === sessionId)) {
        return state;
      }
      return {
        terminalEvents: [...state.terminalEvents, ...events],
      };
    }),
  resetTerminalEvents: () =>
    set({
      activeSessionId: null,
      dismissedSessionIds: [],
      exportedLog: null,
      frontendFailedSessions: {},
      terminalEvents: [],
      terminalInput: defaultTerminalInput(),
    }),
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),
  setExportedLog: (exportedLog) => set({ exportedLog }),
  setSearchOpen: (searchOpen) => set({ searchOpen }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setSplitMode: (splitMode) => set({ splitMode }),
  setTerminalSearchAddon: (terminalSearchAddon) => set({ terminalSearchAddon }),
  startTerminalSession: (sessionId, events) =>
    set((state) => ({
      activeSessionId: sessionId,
      dismissedSessionIds: state.dismissedSessionIds.filter((id) => id !== sessionId),
      exportedLog: null,
      terminalEvents: [
        ...state.terminalEvents.filter((event) => event.sessionId !== sessionId),
        ...events,
      ],
    })),
  setTerminalEvents: (terminalEvents) => set({ terminalEvents }),
  setTerminalInput: (terminalInput) => set({ terminalInput }),
}));

function appendCoalescedTerminalEvents(
  currentEvents: SshSessionEvent[],
  nextEvents: SshSessionEvent[],
) {
  const terminalEvents = [...currentEvents];
  for (const event of nextEvents) {
    const previous = terminalEvents[terminalEvents.length - 1];
    if (
      previous?.sessionId === event.sessionId &&
      previous.kind === "output" &&
      event.kind === "output"
    ) {
      terminalEvents[terminalEvents.length - 1] = {
        ...previous,
        data: `${previous.data}${event.data}`,
        createdAt: event.createdAt,
      };
      continue;
    }

    terminalEvents.push(event);
  }
  return terminalEvents;
}
export function redactTerminalLog(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => {
      if (
        /(^|\b)(authorization|cookie|proxy-authorization|x-api-key|x-auth-token|password|passphrase|private[-_ ]?key)(\b|:|=)/i.test(
          line,
        )
      ) {
        return "<redacted>";
      }

      return line;
    })
    .join("\n");
}
