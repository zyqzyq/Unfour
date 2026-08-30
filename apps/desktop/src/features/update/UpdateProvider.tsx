import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { checkForUpdate, getUpdateInfo, installUpdate, updaterError } from "./updateApi";
import { recordSuccessfulUpdateCheck, wasUpdateCheckedRecently } from "./updateCheckPolicy";
import type { UpdateMeta, UpdateState } from "./updateTypes";
import { UpdateContext } from "./useUpdate";

const SILENT_CHECK_DELAY_MS = 5000;
let automaticCheckStarted = false;

export function UpdateProvider({ children }: { children: ReactNode }) {
  const [meta, setMeta] = useState<UpdateMeta | null>(null);
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const [dialogOpen, setDialogOpen] = useState(false);
  const mountedRef = useRef(false);
  const busyRef = useRef(false);
  const metaRef = useRef<UpdateMeta | null>(null);
  const stateRef = useRef(state);
  useLayoutEffect(() => { stateRef.current = state; }, [state]);

  useEffect(() => {
    mountedRef.current = true;
    getUpdateInfo()
      .then((value) => {
        metaRef.current = value;
        if (!mountedRef.current) return;
        setMeta(value);
        if (!value.updaterEnabled || value.distribution === "microsoft-store") {
          setState({ kind: "managedByStore" });
        }
      })
      .catch(() => undefined);
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const runCheck = useCallback(async (silent = false) => {
    const currentMeta = metaRef.current;
    if (!currentMeta?.updaterEnabled || currentMeta.distribution !== "standard") return;
    if (busyRef.current) return;
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
  }, []);

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
    const info = current.kind === "available"
        || current.kind === "downloading"
        || current.kind === "installing"
      ? current.info
      : current.kind === "error"
        ? current.info
        : undefined;
    if (!info) return;

    busyRef.current = true;
    let downloaded = 0;
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
        if (!mountedRef.current) return;
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
    } catch (error) {
      const detail = updaterError(error, "download");
      if (mountedRef.current) setState({ kind: "error", info, ...detail });
    } finally {
      if (flushTimer !== null) clearTimeout(flushTimer);
      busyRef.current = false;
    }
  }, []);

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
    }),
    [dialogOpen, install, meta, openDialog, runCheck, state],
  );
  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}
