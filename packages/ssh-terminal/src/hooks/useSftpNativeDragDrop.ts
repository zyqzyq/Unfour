import { useEffect, useRef, useState, type RefObject } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { uploadSftpFile, type SftpFileEntry, type SftpTransferState } from "@unfour/command-client";

export function useSftpNativeDragDrop({
  connected,
  currentPath,
  entries,
  listRef,
  onError,
  onTransfer,
  sessionId,
  workspaceId,
}: {
  connected: boolean;
  currentPath: string | null;
  entries: SftpFileEntry[];
  listRef: RefObject<HTMLElement | null>;
  onError: (error: unknown, options: { key: string }) => void;
  onTransfer: (transfer: SftpTransferState) => void;
  sessionId: string;
  workspaceId: string;
}) {
  const [dropActive, setDropActive] = useState(false);
  const dropContextRef = useRef({
    currentPath,
    entries,
    sessionId,
    workspaceId,
  });
  dropContextRef.current = {
    currentPath,
    entries,
    sessionId,
    workspaceId,
  };

  useEffect(() => {
    if (!connected || !isTauriRuntime()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const webview = getCurrentWebview();
        const appWindow = getCurrentWindow();
        unlisten = await webview.onDragDropEvent(async (event) => {
          if (cancelled) return;
          const payload = event.payload;
          if (payload.type === "leave") {
            setDropActive(false);
            return;
          }
          if (payload.type === "over" || payload.type === "enter" || payload.type === "drop") {
            const el = listRef.current;
            if (!el) {
              setDropActive(false);
              return;
            }
            const scale = await appWindow.scaleFactor();
            const rect = el.getBoundingClientRect();
            const x = payload.position.x / scale;
            const y = payload.position.y / scale;
            const inside =
              x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
            if (payload.type !== "drop") {
              setDropActive(inside);
              return;
            }
            setDropActive(false);
            const { currentPath: path, entries: listing, sessionId: sid, workspaceId: wid } =
              dropContextRef.current;
            if (!inside || !path) return;
            for (const localPath of payload.paths) {
              const name = localFileName(localPath);
              if (!name) continue;
              const existing = listing.find((entry) => entry.name === name);
              if (existing && existing.kind !== "file") continue;
              try {
                const transfer = await uploadSftpFile({
                  workspaceId: wid,
                  sessionId: sid,
                  localPath,
                  remotePath: joinRemotePath(path, name),
                  overwrite: Boolean(existing),
                });
                onTransfer(transfer);
              } catch (error) {
                onError(error, { key: "ssh.sftp.uploadDroppedFailed" });
                break;
              }
            }
          }
        });
      } catch {
        // Browser / non-Tauri runtimes skip native drag-drop.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [connected, listRef, onError, onTransfer]);

  return dropActive;
}

function joinRemotePath(parent: string, name: string) {
  return `${parent === "/" ? "" : parent}/${name}`;
}

function localFileName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function isTauriRuntime() {
  return (
    typeof window !== "undefined" &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}
