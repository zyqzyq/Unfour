import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { checkForUpdate, getUpdateInfo, installUpdate, listenForInstalledUpdate, restartAfterUpdate, updaterError } from "./updateApi";
import { recordSuccessfulUpdateCheck, wasUpdateCheckedRecently } from "./updateCheckPolicy";
import type { UpdateMeta, UpdateState } from "./updateTypes";
import { UpdateContext } from "./useUpdate";

const SILENT_CHECK_DELAY_MS = 5000;
let automaticCheckStarted = false;

export function UpdateProvider({ children }: { children: ReactNode }) {
  const [meta, setMeta] = useState<UpdateMeta | null>(null);
  const [state, renderState] = useState<UpdateState>({ kind: "idle" });
  const [dialogOpen, setDialogOpen] = useState(false);
  const mountedRef = useRef(false);
  const busyRef = useRef(false);
  const metaRef = useRef<UpdateMeta | null>(null);
  const stateRef = useRef(state);
  const setState = useCallback((next: UpdateState) => {
    if (!mountedRef.current || (stateRef.current.kind === "installed" && next.kind !== "installed")) return;
    stateRef.current = next;
    renderState(next);
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    async function load() {
      try {
        // Subscribe before reading the snapshot so a reload during installation
        // observes completion even when this WebView did not initiate the install.
        unlisten = await listenForInstalledUpdate((info) => {
          if (!disposed) setState({ kind: "installed", info });
        });
      } catch {
        // The metadata snapshot and initiating call still work without events.
      }
      if (disposed) { unlisten?.(); return; }
      try {
        const value = await getUpdateInfo();
        if (disposed) return;
        metaRef.current = value;
        setMeta(value);
        if (!value.updaterEnabled || value.distribution === "microsoft-store") {
          setState({ kind: "managedByStore" });
        } else if (value.installedVersion && stateRef.current.kind !== "installed") {
          setState({ kind: "installed", info: {
            version: value.installedVersion, currentVersion: value.version, date: null, body: null,
          } });
        }
      } catch {
        // The updater stays unavailable when native metadata cannot be read.
      }
    }
    void load();
    return () => {
      disposed = true;
      mountedRef.current = false;
      unlisten?.();
    };
  }, [setState]);

  const runCheck = useCallback(async (silent = false) => {
    const currentMeta = metaRef.current;
    if (!currentMeta?.updaterEnabled || currentMeta.distribution !== "standard") return;
    if (busyRef.current || stateRef.current.kind === "installed") return;
    busyRef.current = true;
    if (mountedRef.current) setState({ kind: "checking" });
    try {
      const info = await checkForUpdate();
      recordSuccessfulUpdateCheck();
      if (mountedRef.current) {
        setState(info ? { kind: "available", info } : { kind: "upToDate" });
      }
    } catch (error) {
      const detail = updaterError(error, "check");
      if (mountedRef.current) setState({ kind: "error", ...detail });
      if (!silent) setDialogOpen(true);
    } finally {
      busyRef.current = false;
    }
  }, [setState]);

  useEffect(() => {
    if (!meta?.updaterEnabled || meta.distribution !== "standard") return undefined;
    const timer = window.setTimeout(() => {
      if (automaticCheckStarted || wasUpdateCheckedRecently()) return;
      automaticCheckStarted = true;
      void runCheck(true);
    }, SILENT_CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [meta, runCheck]);

  const install = useCallback(async () => {
    const currentMeta = metaRef.current;
    if (!currentMeta?.updaterEnabled || currentMeta.distribution !== "standard") return;
    if (busyRef.current) return;
    const current = stateRef.current;
    const info = current.kind === "available" || current.kind === "error" ? current.info : undefined;
    if (!info) return;

    busyRef.current = true;
    let downloaded = 0;
    let acceptingProgress = true;
    let total: number | null = null;
    const throttleMs = 120;
    let lastRender = 0;
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const render = () => {
      flushTimer = null;
      lastRender = Date.now();
      if (mountedRef.current) {
        setState({ kind: "downloading", info, downloaded, total });
      }
    };
    const scheduleRender = () => {
      if (flushTimer !== null) return;
      flushTimer = setTimeout(render, Math.max(0, throttleMs - (Date.now() - lastRender)));
    };
    setState({ kind: "downloading", info, downloaded, total });
    try {
      await installUpdate((event) => {
        if (!mountedRef.current || !acceptingProgress) return;
        if (event.event === "started") {
          total = Number.isFinite(event.contentLength) && event.contentLength! >= 0
            ? event.contentLength
            : null;
          if (flushTimer !== null) clearTimeout(flushTimer);
          render();
        } else if (event.event === "progress") {
          if (Number.isFinite(event.chunkLength) && event.chunkLength >= 0) {
            downloaded += event.chunkLength;
          }
          scheduleRender();
        } else if (event.event === "downloaded" || event.event === "installing") {
          if (flushTimer !== null) clearTimeout(flushTimer);
          flushTimer = null;
          setState({ kind: "installing", info });
        }
      });
      if (mountedRef.current && !currentMeta.installerExitsApp) {
        setState({ kind: "installed", info });
      }
    } catch (error) {
      const detail = updaterError(error, "download");
      if (mountedRef.current) setState({ kind: "error", info, ...detail });
    } finally {
      acceptingProgress = false;
      if (flushTimer !== null) clearTimeout(flushTimer);
      busyRef.current = false;
    }
  }, [setState]);

  const restart = useCallback(async () => {
    const current = stateRef.current;
    if (current.kind !== "installed" || busyRef.current) return;
    busyRef.current = true;
    try {
      await restartAfterUpdate();
    } catch (error) {
      if (mountedRef.current) {
        setState({ ...current, restartError: updaterError(error, "installer").message });
      }
    } finally {
      busyRef.current = false;
    }
  }, [setState]);

  const openDialog = useCallback(() => setDialogOpen(true), []);
  const value = useMemo(
    () => ({
      meta,
      state,
      dialogOpen,
      setDialogOpen,
      openDialog,
      check: () => runCheck(false),
      install,
      restart,
    }),
    [dialogOpen, install, meta, openDialog, restart, runCheck, state],
  );
  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}
