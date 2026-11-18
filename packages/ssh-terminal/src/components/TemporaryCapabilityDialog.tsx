import { useEffect, useRef, useState } from "react";
import {
  cancelTemporarySshProbe, getAppInfo, getTemporarySshProbe, startTemporarySshProbe, startTemporarySshTcpProbe,
  type AppInfo, type TemporaryProbeKind, type TemporaryProbeReport,
} from "@unfour/command-client";
import {
  Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, DialogXClose, Input, Select, useFeedbackErrorHandler, useI18n,
  type TFunction,
} from "@unfour/ui";
import { useSshClipboardWrite } from "../hooks/useSshClipboardWrite";
import { formatTaskRunError } from "../model/errors";
import { sanitizeTemporaryProbeReport, serializeTemporaryCapabilityReport, temporaryTcpTarget } from "./temporary-capability-report";

type Scope = {
  alive: boolean;
  action: boolean;
  reading: boolean;
  ready: boolean;
  stopRequested: boolean;
  current: TemporaryProbeReport | null;
  timer?: ReturnType<typeof setTimeout>;
  refresh: () => Promise<void>;
};
type Props = { workspaceId: string; handle: string; label: string; connected: boolean; terminalBusy?: boolean; terminalRecoveryRequired?: boolean };

/** Key all local results/authorization to one live workspace + session. */
export function TemporaryCapabilityDialog(props: Props) {
  return <CapabilityDialogScope key={`${props.workspaceId}:${props.handle}`} {...props} />;
}

function CapabilityDialogScope({ workspaceId, handle, label, connected, terminalBusy = false, terminalRecoveryRequired = false }: Props) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();
  const copy = useSshClipboardWrite();
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<TemporaryProbeKind>("transport");
  const usesLoggedInTerminal = ["terminal", "transport", "curl"].includes(kind);
  const [authorized, setAuthorized] = useState(false);
  const [fileAuthorized, setFileAuthorized] = useState(false);
  const [terminalReady, setTerminalReady] = useState(false);
  const [tcpHost, setTcpHost] = useState("");
  const [tcpPort, setTcpPort] = useState("");
  const [loading, setLoading] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [failed, setFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [reports, setReports] = useState<TemporaryProbeReport[]>([]);
  const [latest, setLatest] = useState<TemporaryProbeReport | null>(null);
  const [app, setApp] = useState<AppInfo | null>(null);
  const scope = useRef<Scope | null>(null);
  const errorHandler = useRef(handleError);
  useEffect(() => { errorHandler.current = handleError; }, [handleError]);
  useEffect(() => {
    if (connected && !terminalBusy && !terminalRecoveryRequired) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Confirmation belongs to one available live terminal.
    setTerminalReady(false);
  }, [connected, terminalBusy, terminalRecoveryRequired]);

  function openDialog() {
    setLoading(true); setStarting(false); setStopping(false); setFailed(false);
    setAuthorized(false); setFileAuthorized(false); setTerminalReady(false); setCopied(false);
    setTcpHost(""); setTcpPort("");
    setOpen(true);
  }

  useEffect(() => {
    if (!open) return;
    const current: Scope = { alive: true, action: false, reading: false, ready: false, stopRequested: false, current: null, refresh: async () => undefined };
    scope.current = current;
    const fail = () => {
      // Never send transport errors, endpoint text or report contents to logs.
      errorHandler.current({ message: "TEMPORARY_PROBE_ACTION_FAILED" }, { key: "ssh.temporary.probe.actionFailed" });
    };
    current.refresh = async () => {
      if (!current.alive || current.reading) return;
      clearTimeout(current.timer);
      current.reading = true;
      try {
        const report = await getTemporarySshProbe(workspaceId, handle);
        if (!current.alive) return;
        current.current = report;
        current.ready = true;
        setLatest(report); setFailed(false);
        if (report) setReports((previous) => [...previous.filter((entry) => entry.kind !== report.kind), report]);
        if (report?.status === "running") current.timer = setTimeout(() => void current.refresh(), 500);
        else setStopping(false);
      } catch {
        if (current.alive) { current.ready = false; setFailed(true); fail(); }
      } finally {
        current.reading = false;
        if (current.alive) setLoading(false);
      }
    };
    void current.refresh();
    void getAppInfo().then((info) => { if (current.alive) setApp(info); }).catch(() => {
      // Build metadata is optional; the report explicitly represents it as null.
      if (current.alive) setApp(null);
    });
    return () => {
      current.alive = false;
      clearTimeout(current.timer);
      if (current.current?.status === "running") {
        void cancelTemporarySshProbe(workspaceId, handle, current.current.runId).catch((error: unknown) => {
          // Closing the parent can remove its entry before this cleanup runs.
          const message = error && typeof error === "object" && "message" in error ? error.message : error;
          if (message !== "TEMPORARY_NOT_FOUND" && message !== "validation error: TEMPORARY_NOT_FOUND") fail();
        });
      }
    };
  }, [open, workspaceId, handle]);

  async function start() {
    const current = scope.current;
    if (!connected || terminalBusy || !current?.alive || !current.ready || current.action || current.reading || current.current?.status === "running" || !authorized || (kind === "sftp" && !fileAuthorized) || (usesLoggedInTerminal && (!terminalReady || terminalRecoveryRequired))) return;
    const target = temporaryTcpTarget(tcpHost, tcpPort);
    if (kind === "tcp" && !target) return;
    current.action = true;
    current.stopRequested = false;
    setStarting(true); setCopied(false); setFailed(false);
    try {
      const report = kind === "tcp" && target
        ? await startTemporarySshTcpProbe(workspaceId, handle, target.host, target.port, true)
        : await startTemporarySshProbe(workspaceId, handle, kind, true);
      current.current = report;
      if (!current.alive || current.stopRequested) {
        if (report.status === "running") await cancelTemporarySshProbe(workspaceId, handle, report.runId);
        if (current.alive) await current.refresh();
        return;
      }
      setLatest(report);
      setReports((previous) => [...previous.filter((entry) => entry.kind !== report.kind), report]);
      if (report.status === "running") current.timer = setTimeout(() => void current.refresh(), 500);
    } catch {
      if (current.alive) {
        setFailed(true);
        handleError({ message: "TEMPORARY_PROBE_ACTION_FAILED" }, { key: "ssh.temporary.probe.actionFailed" });
        // A timeout can hide a successful start; re-read before permitting another.
        current.ready = false;
        await current.refresh();
        if (current.alive && current.stopRequested && current.current?.status === "running") {
          await cancelTemporarySshProbe(workspaceId, handle, current.current.runId).catch(() => {
            current.stopRequested = false;
            if (current.alive) {
              setStopping(false); setFailed(true);
              handleError({ message: "TEMPORARY_PROBE_ACTION_FAILED" }, { key: "ssh.temporary.probe.actionFailed" });
            }
          });
          await current.refresh();
        }
      }
    } finally {
      current.action = false;
      if (current.alive) { setStarting(false); setAuthorized(false); setFileAuthorized(false); setTerminalReady(false); if (kind === "tcp") { setTcpHost(""); setTcpPort(""); } }
    }
  }

  async function stop() {
    const current = scope.current;
    if (!current?.alive || current.stopRequested) return;
    current.stopRequested = true;
    setStopping(true);
    if (current.action) return; // A late start is cancelled by its own run ID.
    const report = current.current;
    if (!report || report.status !== "running") { setStopping(false); return; }
    try {
      await cancelTemporarySshProbe(workspaceId, handle, report.runId);
      if (current.alive) await current.refresh();
    } catch {
      if (current.alive) {
        current.stopRequested = false; setStopping(false); setFailed(true);
        handleError({ message: "TEMPORARY_PROBE_ACTION_FAILED" }, { key: "ssh.temporary.probe.actionFailed" });
      }
    }
  }

  const busy = starting || latest?.status === "running";
  const safeLatest = latest ? sanitizeTemporaryProbeReport(latest) : null;
  const failures = safeLatest?.status !== "running" ? safeLatest?.checks.filter((check) =>
    check.status !== "pass" && check.status !== "cancelled",
  ) : undefined;
  const primaryFailure = failures?.find((check) => !check.id.endsWith("-recovery")) ?? failures?.[0];
  const curlNotChecked = safeLatest?.kind === "curl" && safeLatest.status !== "running"
    && safeLatest.checks.some((check) => check.id.startsWith("transport-"))
    && !safeLatest.checks.some((check) => check.id === "curl-tools");
  const reportText = serializeTemporaryCapabilityReport(reports, app);
  return <>
    <Button variant="secondary" onClick={openDialog}>{t("ssh.temporary.probe.title")}</Button>
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent>
        <DialogHeader>
          <div className="min-w-0">
            <DialogTitle>{t("ssh.temporary.probe.title")}</DialogTitle>
            <DialogDescription className="truncate" title={label}>{label}</DialogDescription>
          </div>
          <DialogXClose label={t("ssh.temporary.probe.close")} />
        </DialogHeader>
        <DialogBody className="space-y-3">
          <p>{t("ssh.temporary.probe.help")}</p>
          {primaryFailure && <div role="alert" className="space-y-1 text-[var(--u-color-danger)]">
            <p className="font-medium">{t("ssh.temporary.probe.issue", { check: t(`ssh.temporary.probe.check.${primaryFailure.id}`, t("ssh.temporary.probe.check.unknown-check")) })}</p>
            {probeCheckDescription(primaryFailure.code, t) && <p>{probeCheckDescription(primaryFailure.code, t)}</p>}
          </div>}
          {curlNotChecked && <p role="note">{t("ssh.temporary.probe.curlNotChecked")}</p>}
          {!connected && <p role="note">{t("ssh.temporary.probe.disconnected")}</p>}
          <label className="block space-y-1">
            <span>{t("ssh.temporary.probe.kindLabel")}</span>
            <Select aria-label={t("ssh.temporary.probe.kindLabel")} disabled={!connected || busy || loading} value={kind}
              onChange={(event) => { setKind(event.target.value as TemporaryProbeKind); setAuthorized(false); setFileAuthorized(false); setTerminalReady(false); setCopied(false); }}
              options={(["transport", "curl", "terminal", "exec", "channels", "sftp", "tcp"] as const).map((value) => ({ value, label: t(`ssh.temporary.probe.kind.${value}`) }))} />
          </label>
          <p className="text-[var(--u-color-text-muted)]">{t(`ssh.temporary.probe.description.${kind}`)}</p>
          {kind === "tcp" && <div className="space-y-2">
            <div className="grid grid-cols-[1fr_100px] gap-2">
              <label className="space-y-1"><span>{t("ssh.temporary.probe.tcpHost")}</span><Input disabled={!connected || busy || loading} value={tcpHost} onChange={(event) => { setTcpHost(event.target.value); setAuthorized(false); }} autoComplete="off" spellCheck={false} /></label>
              <label className="space-y-1"><span>{t("ssh.temporary.probe.tcpPort")}</span><Input disabled={!connected || busy || loading} value={tcpPort} onChange={(event) => { setTcpPort(event.target.value); setAuthorized(false); }} autoComplete="off" inputMode="numeric" /></label>
            </div>
            <p className="text-[var(--u-color-text-muted)]">{t("ssh.temporary.probe.tcpHelp")}</p>
            {(tcpHost || tcpPort) && !temporaryTcpTarget(tcpHost, tcpPort) && <p role="status">{t("ssh.temporary.probe.tcpInvalid")}</p>}
          </div>}
          <label className="flex items-start gap-2"><input type="checkbox" checked={authorized} disabled={!connected || busy || loading} onChange={(event) => setAuthorized(event.target.checked)} /><span>{t("ssh.temporary.probe.authorize")}</span></label>
          {kind === "sftp" && <label className="flex items-start gap-2"><input type="checkbox" checked={fileAuthorized} disabled={!connected || busy || loading} onChange={(event) => setFileAuthorized(event.target.checked)} /><span>{t("ssh.temporary.probe.authorizeFile")}</span></label>}
          {usesLoggedInTerminal && <label className="flex items-start gap-2"><input type="checkbox" checked={terminalReady} disabled={!connected || busy || loading || terminalBusy || terminalRecoveryRequired} onChange={(event) => setTerminalReady(event.target.checked)} /><span>{t("ssh.tasks.run.terminalReady")}</span></label>}
          {usesLoggedInTerminal && terminalRecoveryRequired && <p role="alert">{t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_RECOVERY_REQUIRED")}</p>}
          {terminalBusy && !busy && <p role="note">{t("ssh.temporary.terminalTaskBusy")}</p>}
          <div role="status" aria-live="polite">{t(`ssh.temporary.probe.runStatus.${loading ? "loading" : stopping ? "stopping" : starting ? "running" : latest?.status ?? "idle"}`)}</div>
          {failed && <div role="alert" className="flex items-center gap-2"><span>{t("ssh.temporary.probe.actionFailed")}</span><Button variant="secondary" onClick={() => void scope.current?.refresh()}>{t("ssh.temporary.probe.refresh")}</Button></div>}
          {safeLatest && <div className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
            <p className="font-medium">{t("ssh.temporary.probe.latest", { kind: t(`ssh.temporary.probe.kind.${safeLatest.kind}`) })}</p>
            {safeLatest.checks.map((check, index) => <div key={`${check.id}-${index}`} className="flex flex-wrap items-center gap-x-3 gap-y-1">
              <span>{t(`ssh.temporary.probe.check.${check.id}`, t("ssh.temporary.probe.check.unknown-check"))}</span>
              <span>{t(`ssh.temporary.probe.result.${check.status}`)}</span>
              <code className="select-text text-xs">{check.code}</code>
              {check !== primaryFailure && probeCheckDescription(check.code, t) && <span>{probeCheckDescription(check.code, t)}</span>}
              {check.expectedExitCode !== null && <span>{t("ssh.temporary.probe.exitCode", { expected: check.expectedExitCode, actual: check.actualExitCode ?? "—" })}</span>}
            </div>)}
            {safeLatest.cleanup && <p>{t(`ssh.temporary.probe.cleanup.${safeLatest.cleanup}`)}</p>}
            {safeLatest.probeFile && <p className="select-text break-all font-mono text-xs">{safeLatest.probeFile}</p>}
          </div>}
          <p className="text-[var(--u-color-text-muted)]">{t("ssh.temporary.probe.manual")}</p>
          <p className="text-[var(--u-color-text-muted)]">{t("ssh.temporary.probe.reportHelp")}</p>
          {reports.length > 0 && <details><summary>{t("ssh.temporary.probe.preview")}</summary><pre data-testid="temporary-capability-report" className="max-h-48 select-text overflow-auto whitespace-pre-wrap break-all text-xs">{reportText}</pre></details>}
        </DialogBody>
        <DialogFooter>
          <Button variant="secondary" disabled={reports.length === 0} onClick={() => { const current = scope.current; void copy(reportText).then((ok) => { if (current?.alive && current === scope.current && ok) setCopied(true); }); }}>{t(copied ? "ssh.temporary.probe.copied" : "ssh.temporary.probe.copy")}</Button>
          {busy ? <Button variant="secondary" disabled={stopping} onClick={() => void stop()}>{t("ssh.temporary.probe.stop")}</Button> : <Button disabled={!connected || terminalBusy || loading || failed || !authorized || (kind === "sftp" && !fileAuthorized) || (usesLoggedInTerminal && (!terminalReady || terminalRecoveryRequired)) || (kind === "tcp" && !temporaryTcpTarget(tcpHost, tcpPort))} onClick={() => void start()}>{t("ssh.temporary.probe.run")}</Button>}
          <Button variant="secondary" onClick={() => setOpen(false)}>{t("ssh.temporary.probe.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </>;
}

function probeCheckDescription(code: string, t: TFunction): string | null {
  if (["CURL_MISSING", "TERMINAL_TOOLS_MISSING", "CURL_OPTIONS_UNSUPPORTED", "TERMINAL_PROTOCOL_ERROR"].includes(code)) return t(`ssh.temporary.probe.errors.${code}`);
  if (code.startsWith("SSH_TUNNEL_") || code.startsWith("SSH_TERMINAL_")) return t(`network.errors.${code}`, t("ssh.temporary.probe.actionFailed"));
  if (code.startsWith("SSH_TASK_")) return formatTaskRunError(code, t);
  if (["TERMINAL_RECOVERY_REQUIRED", "TERMINAL_NOT_READY", "INTERACTIVE_LOGIN_PROMPT"].includes(code)) return t(`ssh.tasks.run.errors.SSH_TASK_${code}`);
  return null;
}
