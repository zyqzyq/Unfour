import {
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { ChevronLeft, FolderOpen, Loader2 } from "lucide-react";
import {
  listSftpTransfers,
  registerSftpTransferChannel,
  type SftpTransferState,
  type SshSessionSummary,
} from "@unfour/command-client";
import { IconButton, cn, useI18n } from "@unfour/ui";
import {
  DEFAULT_SFTP_PANEL_WIDTH,
  MAX_SFTP_PANEL_WIDTH,
  MIN_SFTP_PANEL_WIDTH,
  clampSftpPanelWidth,
  maxSftpPanelWidth,
  useSftpStore,
} from "../model/sftp-state";
import { SftpPanel } from "./SftpPanel";

const EMPTY_TRANSFERS: SftpTransferState[] = [];

export function SftpWorkspace({
  children,
  session,
}: {
  children: ReactNode;
  session: SshSessionSummary | null;
}) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [availableWidth, setAvailableWidth] = useState(Number.POSITIVE_INFINITY);
  const dragRef = useRef<{ pointerId: number; startWidth: number; startX: number } | null>(
    null,
  );
  const panelWidth = useSftpStore((state) => state.panelWidth);
  const setPanelOpen = useSftpStore((state) => state.setPanelOpen);
  const setPanelWidth = useSftpStore((state) => state.setPanelWidth);
  const setTransfers = useSftpStore((state) => state.setTransfers);
  const upsertTransfer = useSftpStore((state) => state.upsertTransfer);
  const tab = useSftpStore((state) =>
    session ? state.tabs[session.sessionId] : undefined,
  );
  const transfers = useSftpStore((state) =>
    session ? state.transfers[session.sessionId] ?? EMPTY_TRANSFERS : EMPTY_TRANSFERS,
  );
  const open = Boolean(
    session && tab?.connectionId === session.connectionId && tab.open,
  );
  const runningTransfers = transfers.filter((transfer) =>
    ["pending", "running"].includes(transfer.status),
  );
  const renderedPanelWidth = clampSftpPanelWidth(panelWidth, availableWidth);
  const renderedPanelMaxWidth = maxSftpPanelWidth(availableWidth);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const updateAvailableWidth = () => {
      const width = host.getBoundingClientRect().width;
      setAvailableWidth(width > 0 ? width : Number.POSITIVE_INFINITY);
    };
    updateAvailableWidth();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(updateAvailableWidth);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    registerSftpTransferChannel(upsertTransfer).then((nextDispose) => {
      if (disposed) nextDispose();
      else dispose = nextDispose;
    });
    return () => {
      disposed = true;
      dispose?.();
    };
  }, [upsertTransfer]);

  useEffect(() => {
    if (!session) return;
    let cancelled = false;
    listSftpTransfers({ workspaceId: session.workspaceId, sessionId: session.sessionId })
      .then((items) => {
        if (!cancelled) setTransfers(session.sessionId, items);
      })
      .catch(() => {
        // Transfer history is supplemental; a failure must not affect the PTY.
      });
    return () => {
      cancelled = true;
    };
  }, [session, setTransfers]);

  function resizeTo(nextWidth: number) {
    const available = hostRef.current?.getBoundingClientRect().width ?? Number.POSITIVE_INFINITY;
    setPanelWidth(clampSftpPanelWidth(nextWidth, available));
  }

  function startResize(event: ReactPointerEvent<HTMLDivElement>) {
    dragRef.current = {
      pointerId: event.pointerId,
      startWidth: renderedPanelWidth,
      startX: event.clientX,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveResize(event: ReactPointerEvent<HTMLDivElement>) {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    resizeTo(drag.startWidth + drag.startX - event.clientX);
  }

  function stopResize(event: ReactPointerEvent<HTMLDivElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) return;
    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  return (
    <div className="relative flex min-h-0 min-w-0 flex-1" ref={hostRef}>
      <div className="relative flex min-h-0 min-w-0 flex-1">
        {children}
        {session && !open ? (
          <IconButton
            aria-expanded={false}
            className="absolute right-0 top-1/2 z-20 h-10 w-6 -translate-y-1/2 rounded-r-none border border-r-0 border-[var(--u-color-border-strong)] bg-[var(--u-color-surface)] shadow-sm hover:border-[var(--u-color-focus)]"
            label={t("ssh.sftp.openPanel")}
            onClick={() => setPanelOpen(session.sessionId, session.connectionId, true)}
            tooltip={t("ssh.sftp.openPanelTooltip")}
          >
            <span className="relative">
              <FolderOpen size={14} />
              <ChevronLeft
                className="absolute -bottom-1.5 -right-1.5 rounded-full bg-[var(--u-color-surface)]"
                size={9}
              />
            </span>
          </IconButton>
        ) : null}
        {session && !open && runningTransfers.length > 0 ? (
          <button
            className="absolute bottom-2 right-2 z-20 flex items-center gap-1.5 rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 py-1 text-[11px] text-[var(--u-color-text-muted)] shadow-sm transition-colors hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]"
            onClick={() => setPanelOpen(session.sessionId, session.connectionId, true)}
            type="button"
          >
            <Loader2 className="animate-spin" size={12} />
            {t("ssh.sftp.activeTransfers", { count: runningTransfers.length })}
          </button>
        ) : null}
      </div>
      {session && open ? (
        <>
          <div
            aria-label={t("ssh.sftp.resizePanel")}
            aria-orientation="vertical"
            aria-valuemax={Math.min(MAX_SFTP_PANEL_WIDTH, renderedPanelMaxWidth)}
            aria-valuemin={MIN_SFTP_PANEL_WIDTH}
            aria-valuenow={renderedPanelWidth}
            className={cn(
              "group relative z-20 w-1.5 shrink-0 cursor-col-resize touch-none bg-transparent outline-none",
              "focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--u-color-focus)]",
            )}
            onDoubleClick={() => resizeTo(DEFAULT_SFTP_PANEL_WIDTH)}
            onKeyDown={(event) => {
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                resizeTo(renderedPanelWidth + 16);
              } else if (event.key === "ArrowRight") {
                event.preventDefault();
                resizeTo(renderedPanelWidth - 16);
              } else if (event.key === "Home") {
                event.preventDefault();
                resizeTo(DEFAULT_SFTP_PANEL_WIDTH);
              }
            }}
            onPointerCancel={stopResize}
            onPointerDown={startResize}
            onPointerMove={moveResize}
            onPointerUp={stopResize}
            role="separator"
            tabIndex={0}
            title={t("ssh.sftp.resizePanelTooltip")}
          >
            <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-[var(--u-color-border-strong)] transition-colors duration-150 group-hover:bg-[var(--u-color-focus)]" />
          </div>
          <aside
            aria-label={t("ssh.sftp.panelTitle")}
            className="flex min-h-0 shrink-0 flex-col border-l border-[var(--u-color-border)] bg-[var(--u-color-surface)]"
            style={{ width: renderedPanelWidth }}
          >
            <SftpPanel
              onClose={() => setPanelOpen(session.sessionId, session.connectionId, false)}
              session={session}
            />
          </aside>
        </>
      ) : null}
    </div>
  );
}
