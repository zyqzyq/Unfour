import { useCallback, useEffect, useRef, useState, type ReactNode, type ComponentProps } from "react";
import { Button, Tabs, useI18n } from "@unfour/ui";
import {
  listTemporarySsh, claimTemporarySsh, startTemporarySsh, cancelTemporarySsh,
  closeTemporarySshWorkspace, temporarySshLabel, readTemporarySshOutput,
  type TemporarySshSummary, type SshSessionEvent,
} from "@unfour/command-client";
import { TerminalPane } from "./TerminalPane";

function createScope(workspaceId: string) {
  return { workspaceId, alive: true, connecting: new Set<string>(), closing: new Set<string>(), closed: new Set<string>() };
}

function isMissingTemporary(error: unknown) {
  const message = error && typeof error === "object" && "message" in error ? error.message : error;
  return message === "TEMPORARY_NOT_FOUND" || message === "validation error: TEMPORARY_NOT_FOUND";
}

export function TemporaryTerminalWorkspace({ workspaceId, autoClaim = false, surfaceActive = true, onSelectTemporary, children }: {
  workspaceId: string;
  autoClaim?: boolean;
  surfaceActive?: boolean;
  onSelectTemporary?: () => void;
  children: ReactNode | ((temporaryActive: boolean, showSaved: () => void) => ReactNode);
}) {
  const { t } = useI18n();
  const [requests, setRequests] = useState<TemporarySshSummary[]>([]);
  const [selected, setSelected] = useState("saved");
  const [events, setEvents] = useState<Record<string, SshSessionEvent[]>>({});
  const [failed, setFailed] = useState(false);
  const [pollFailed, setPollFailed] = useState(false);
  const [connecting, setConnecting] = useState<string[]>([]);
  const attempted = useRef(new Set<string>());
  const showSaved = useCallback(() => setSelected("saved"), []);
  const scope = useRef(createScope(workspaceId));
  const deferredClose = useRef<{ workspaceId: string; timer: ReturnType<typeof setTimeout> } | null>(null);

  useEffect(() => {
    // StrictMode's immediate remount must not close an existing live session.
    if (deferredClose.current?.workspaceId === workspaceId) clearTimeout(deferredClose.current.timer);
    const current = createScope(workspaceId);
    scope.current = current;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        // Only retire tombstones after a read begun after that close sees them
        // absent. An older in-flight list must never resurrect a closed tab.
        const closedBeforeRead = new Set(current.closed);
        const listed = await listTemporarySsh(workspaceId);
        if (!current.alive) return;
        const next = listed.filter((item) => !current.closed.has(item.handle));
        for (const handle of closedBeforeRead) {
          if (!current.connecting.has(handle) && !listed.some((item) => item.handle === handle)) current.closed.delete(handle);
        }
        setRequests(next);
        setEvents((previous) => Object.fromEntries(Object.entries(previous).filter(([id]) => next.some((item) => item.handle === id))));
        // Polling is single-flight; notifications/readiness are never storage.
        let outputFailed = false;
        for (const item of next.filter((item) => item.workspaceId === workspaceId)) {
          if (current.closing.has(item.handle) || current.closed.has(item.handle)) continue;
          try {
            const chunks = await readTemporarySshOutput(workspaceId, item.handle);
            if (!current.alive) return;
            if (current.closing.has(item.handle) || current.closed.has(item.handle)) continue;
            if (chunks.length) setEvents((previous) => {
              const tail = [...(previous[item.handle] ?? []), ...chunks];
              let bytes = tail.reduce((sum, event) => sum + event.data.length, 0);
              while (tail.length > 1 && bytes > 256 * 1024) bytes -= tail.shift()!.data.length;
              return { ...previous, [item.handle]: tail };
            });
          } catch (error) {
            if (!current.alive) return;
            // Expiry/close can legitimately race the destructive output drain.
            // One unavailable terminal must not stop the other terminals.
            if (!current.closing.has(item.handle) && !current.closed.has(item.handle) && !isMissingTemporary(error)) outputFailed = true;
          }
        }
        setPollFailed(outputFailed);
      } catch { if (current.alive) setPollFailed(true); }
      finally { if (current.alive) timer = setTimeout(() => void refresh(), 500); }
    }
    void refresh();
    return () => {
      current.alive = false;
      clearTimeout(timer);
      deferredClose.current = { workspaceId, timer: setTimeout(() => { void closeTemporarySshWorkspace(workspaceId).catch(() => undefined); }, 0) };
    };
  }, [workspaceId]);

  const connect = useCallback(async (handle: string) => {
    const current = scope.current;
    if (!current.alive || current.connecting.has(handle) || current.closing.has(handle) || current.closed.has(handle)) return;
    current.connecting.add(handle);
    setConnecting([...current.connecting]);
    try {
      const claimed = await claimTemporarySsh(workspaceId, handle);
      if (!current.alive || current.closing.has(handle) || current.closed.has(handle)) { await cancelTemporarySsh(workspaceId, handle); return; }
      onSelectTemporary?.();
      setSelected(handle);
      if (claimed.status === "claimed") await startTemporarySsh(workspaceId, handle);
      if (!current.alive) await cancelTemporarySsh(workspaceId, handle);
    } catch { if (current.alive && !current.closing.has(handle) && !current.closed.has(handle)) setFailed(true); }
    finally {
      current.connecting.delete(handle);
      if (current.alive) setConnecting([...current.connecting]);
    }
  }, [workspaceId, onSelectTemporary]);
  useEffect(() => {
    for (const handle of attempted.current) {
      if (!requests.some((item) => item.handle === handle)) attempted.current.delete(handle);
    }
    if (!autoClaim || !surfaceActive) return;
    for (const item of requests) {
      if (item.workspaceId !== null || item.status !== "pending" || attempted.current.has(item.handle)) continue;
      attempted.current.add(item.handle);
      void connect(item.handle);
    }
  }, [autoClaim, surfaceActive, requests, connect]);
  async function close(handle: string) {
    const current = scope.current;
    if (!current.alive || current.closing.has(handle) || current.closed.has(handle)) return;
    current.closing.add(handle);
    try {
      await cancelTemporarySsh(workspaceId, handle);
      if (!current.alive) return;
      current.closed.add(handle);
      setRequests((items) => items.filter((item) => item.handle !== handle));
      setEvents((previous) => Object.fromEntries(Object.entries(previous).filter(([id]) => id !== handle)));
      setSelected((previous) => previous === handle ? "saved" : previous);
    } catch { if (current.alive) setFailed(true); }
    finally { current.closing.delete(handle); }
  }
  const owned = requests.filter((item) => item.workspaceId === workspaceId);
  const active = owned.find((item) => item.handle === selected);
  const session: ComponentProps<typeof TerminalPane>["session"] = active ? {
    sessionId: active.handle, workspaceId, connectionId: "", status: active.status === "connected" ? "connected" : "disconnected",
    host: "", username: "", cols: 120, rows: 32,
  } : null;
  return <>
    {requests.filter((item) => item.workspaceId === null && (!autoClaim || failed)).map((item) => <div key={item.handle} className="flex items-center gap-2 border-b border-[var(--u-color-border)] px-3 py-1 text-xs">
      <span className="min-w-0 truncate" title={temporarySshLabel(item, t("ssh.temporary.title"))}>{temporarySshLabel(item, t("ssh.temporary.title"))}</span>
      <span>{t("ssh.temporary.pending", { seconds: item.expiresInSeconds })}</span>
      <Button disabled={connecting.includes(item.handle)} onClick={() => void connect(item.handle)}>{t("ssh.temporary.claim")}</Button>
      <Button variant="secondary" onClick={() => void close(item.handle)}>{t("ssh.temporary.cancel")}</Button>
    </div>)}
    {owned.length > 0 && <Tabs activeId={active?.handle ?? ""} onSelect={(id) => { onSelectTemporary?.(); setSelected(id); }} onClose={(id) => void close(id)}
      endControl={<Button variant="secondary" onClick={showSaved}>{t("ssh.temporary.saved")}</Button>}
      tabs={owned.map((item) => ({ id: item.handle, title: temporarySshLabel(item, t("network.temporarySession", { id: item.handle.slice(-8) })) }))} />}
    {(failed || pollFailed) && <div role="alert" className="px-3 py-1 text-xs">{t("ssh.temporary.failed")} <Button variant="secondary" onClick={() => { setFailed(false); setPollFailed(false); }}>{t("ssh.temporary.dismiss")}</Button></div>}
    {active ? <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 px-3 py-1 text-xs">
        <span className="min-w-0 truncate" title={temporarySshLabel(active, t("ssh.temporary.title"))}>{temporarySshLabel(active, t("network.temporarySession", { id: active.handle.slice(-8) }))}</span>
        <span>{t(`ssh.temporary.status.${active.status}`)}</span>
        {active.errorCode && <span role="alert">{t(`ssh.temporary.errors.${active.errorCode}`, t("ssh.temporary.failed"))}</span>}
        {active.status === "claimed" && <Button disabled={connecting.includes(active.handle)} onClick={() => void connect(active.handle)}>{t("ssh.temporary.connect")}</Button>}
        <Button variant="secondary" onClick={() => void close(active.handle)}>{t("ssh.temporary.cancel")}</Button>
      </div>
      <TerminalPane key={active.handle} active={surfaceActive} temporary session={session} events={events[active.handle] ?? []} inputDisabled={active.status !== "connected"} />
    </div> : null}
    <div className={active ? "hidden" : "flex min-h-0 flex-1 flex-col"}>{typeof children === "function" ? children(Boolean(active), showSaved) : children}</div>
  </>;
}
