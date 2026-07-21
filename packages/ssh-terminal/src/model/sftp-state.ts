import { create } from "zustand";
import type { SftpTransferState, SftpTransferStatus } from "@unfour/command-client";

export const DEFAULT_SFTP_PANEL_WIDTH = 340;
export const MIN_SFTP_PANEL_WIDTH = 260;
export const MAX_SFTP_PANEL_WIDTH = 960;
export const MIN_TERMINAL_SURFACE_WIDTH = 320;
const SFTP_PANEL_WIDTH_KEY = "unfour.ssh.sftp-panel-width";

type SftpTabState = {
  connectionId: string;
  open: boolean;
  path: string | null;
  /** Primary selection / Shift-range anchor. */
  selectedPath: string | null;
  /** All selected remote paths (includes selectedPath when set). */
  selectedPaths: string[];
};

type SftpUiState = {
  panelWidth: number;
  tabs: Record<string, SftpTabState>;
  transfers: Record<string, SftpTransferState[]>;
  setPanelOpen: (sessionId: string, connectionId: string, open: boolean) => void;
  setPanelPath: (sessionId: string, connectionId: string, path: string | null) => void;
  setPanelWidth: (width: number) => void;
  setSelectedPath: (sessionId: string, connectionId: string, path: string | null) => void;
  setSelectedPaths: (
    sessionId: string,
    connectionId: string,
    paths: string[],
    primaryPath?: string | null,
  ) => void;
  setTransfers: (sessionId: string, transfers: SftpTransferState[]) => void;
  upsertTransfer: (transfer: SftpTransferState) => void;
};

const TERMINAL_STATUSES: SftpTransferStatus[] = ["success", "failed", "cancelled"];

export function isTerminalTransferStatus(status: SftpTransferStatus | string) {
  return TERMINAL_STATUSES.includes(status as SftpTransferStatus);
}

/** Prefer authoritative terminal/progress updates over stale channel frames. */
export function preferFresherTransfer(
  current: SftpTransferState | undefined,
  incoming: SftpTransferState,
): SftpTransferState {
  if (!current || current.transferId !== incoming.transferId) return incoming;
  if (isTerminalTransferStatus(current.status) && !isTerminalTransferStatus(incoming.status)) {
    return current;
  }
  if (isTerminalTransferStatus(incoming.status)) return incoming;
  if (incoming.transferredBytes > current.transferredBytes) return incoming;
  if (
    incoming.transferredBytes === current.transferredBytes &&
    incoming.totalBytes >= current.totalBytes
  ) {
    return incoming;
  }
  return current;
}

export function maxSftpPanelWidth(availableWidth = Number.POSITIVE_INFINITY) {
  return Number.isFinite(availableWidth)
    ? Math.max(
        MIN_SFTP_PANEL_WIDTH,
        Math.min(MAX_SFTP_PANEL_WIDTH, availableWidth - MIN_TERMINAL_SURFACE_WIDTH),
      )
    : MAX_SFTP_PANEL_WIDTH;
}

export function clampSftpPanelWidth(width: number, availableWidth = Number.POSITIVE_INFINITY) {
  return Math.round(
    Math.min(Math.max(width, MIN_SFTP_PANEL_WIDTH), maxSftpPanelWidth(availableWidth)),
  );
}

function readStoredPanelWidth() {
  if (typeof window === "undefined") return DEFAULT_SFTP_PANEL_WIDTH;
  const value = Number(window.localStorage.getItem(SFTP_PANEL_WIDTH_KEY));
  return Number.isFinite(value) && value > 0
    ? clampSftpPanelWidth(value)
    : DEFAULT_SFTP_PANEL_WIDTH;
}

function tabForConnection(
  tabs: Record<string, SftpTabState>,
  sessionId: string,
  connectionId: string,
) {
  const current = tabs[sessionId];
  if (current?.connectionId === connectionId) {
    return {
      ...current,
      selectedPaths: current.selectedPaths ?? (current.selectedPath ? [current.selectedPath] : []),
    };
  }
  return {
    connectionId,
    open: false,
    path: null,
    selectedPath: null,
    selectedPaths: [],
  };
}

export const useSftpStore = create<SftpUiState>((set) => ({
  panelWidth: readStoredPanelWidth(),
  tabs: {},
  transfers: {},
  setPanelOpen: (sessionId, connectionId, open) =>
    set((state) => ({
      tabs: {
        ...state.tabs,
        [sessionId]: {
          ...tabForConnection(state.tabs, sessionId, connectionId),
          open,
        },
      },
    })),
  setPanelPath: (sessionId, connectionId, path) =>
    set((state) => ({
      tabs: {
        ...state.tabs,
        [sessionId]: {
          ...tabForConnection(state.tabs, sessionId, connectionId),
          path,
          selectedPath: null,
          selectedPaths: [],
        },
      },
    })),
  setPanelWidth: (width) => {
    const next = clampSftpPanelWidth(width);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(SFTP_PANEL_WIDTH_KEY, String(next));
    }
    set({ panelWidth: next });
  },
  setSelectedPath: (sessionId, connectionId, selectedPath) =>
    set((state) => ({
      tabs: {
        ...state.tabs,
        [sessionId]: {
          ...tabForConnection(state.tabs, sessionId, connectionId),
          selectedPath,
          selectedPaths: selectedPath ? [selectedPath] : [],
        },
      },
    })),
  setSelectedPaths: (sessionId, connectionId, paths, primaryPath) =>
    set((state) => {
      const unique = [...new Set(paths)];
      const selectedPath =
        primaryPath === undefined
          ? (unique[unique.length - 1] ?? null)
          : primaryPath;
      return {
        tabs: {
          ...state.tabs,
          [sessionId]: {
            ...tabForConnection(state.tabs, sessionId, connectionId),
            selectedPath,
            selectedPaths: unique,
          },
        },
      };
    }),
  setTransfers: (sessionId, transfers) =>
    set((state) => {
      const current = state.transfers[sessionId] ?? [];
      const byId = new Map(current.map((item) => [item.transferId, item]));
      const merged = transfers.map((incoming) =>
        preferFresherTransfer(byId.get(incoming.transferId), incoming),
      );
      for (const item of current) {
        if (!merged.some((entry) => entry.transferId === item.transferId)) {
          merged.push(item);
        }
      }
      merged.sort((left, right) => right.startedAt.localeCompare(left.startedAt));
      return {
        transfers: { ...state.transfers, [sessionId]: merged },
      };
    }),
  upsertTransfer: (transfer) =>
    set((state) => {
      const current = state.transfers[transfer.sessionId] ?? [];
      const existing = current.find((item) => item.transferId === transfer.transferId);
      const next = preferFresherTransfer(existing, transfer);
      return {
        transfers: {
          ...state.transfers,
          [transfer.sessionId]: [
            next,
            ...current.filter((item) => item.transferId !== transfer.transferId),
          ],
        },
      };
    }),
}));
