import { create } from "zustand";
import type { SftpTransferState } from "@unfour/command-client";

export const DEFAULT_SFTP_PANEL_WIDTH = 340;
export const MIN_SFTP_PANEL_WIDTH = 260;
export const MAX_SFTP_PANEL_WIDTH = 960;
export const MIN_TERMINAL_SURFACE_WIDTH = 320;
const SFTP_PANEL_WIDTH_KEY = "unfour.ssh.sftp-panel-width";

type SftpTabState = {
  connectionId: string;
  open: boolean;
  path: string | null;
  selectedPath: string | null;
};

type SftpUiState = {
  panelWidth: number;
  tabs: Record<string, SftpTabState>;
  transfers: Record<string, SftpTransferState[]>;
  setPanelOpen: (sessionId: string, connectionId: string, open: boolean) => void;
  setPanelPath: (sessionId: string, connectionId: string, path: string | null) => void;
  setPanelWidth: (width: number) => void;
  setSelectedPath: (sessionId: string, connectionId: string, path: string | null) => void;
  setTransfers: (sessionId: string, transfers: SftpTransferState[]) => void;
  upsertTransfer: (transfer: SftpTransferState) => void;
};

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
  return current?.connectionId === connectionId
    ? current
    : {
        connectionId,
        open: false,
        path: null,
        selectedPath: null,
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
        },
      },
    })),
  setTransfers: (sessionId, transfers) =>
    set((state) => ({
      transfers: { ...state.transfers, [sessionId]: transfers },
    })),
  upsertTransfer: (transfer) =>
    set((state) => {
      const current = state.transfers[transfer.sessionId] ?? [];
      return {
        transfers: {
          ...state.transfers,
          [transfer.sessionId]: [
            transfer,
            ...current.filter((item) => item.transferId !== transfer.transferId),
          ],
        },
      };
    }),
}));
