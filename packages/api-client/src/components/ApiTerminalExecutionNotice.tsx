import { useEffect } from "react";
import { useI18n } from "@unfour/ui";
import type { ApiRouteControls } from "../hooks/useApiRoutes";
import type { ApiRequestTab } from "../model/request-tabs";
import { isTerminalApiError } from "../model/terminal-api";
import type { RequestDraft } from "../model/types";

export function ApiTerminalExecutionNotice({ active, tab, routing, onUpdate }: {
  active: boolean; tab: ApiRequestTab; routing: ApiRouteControls; onUpdate: (patch: Partial<RequestDraft>) => void;
}) {
  const { t } = useI18n();
  const terminal = Boolean(tab.draft.temporaryHandle && tab.draft.temporaryExecutionMode === "terminalCurl");
  const session = routing.temporary.data?.find((entry) => entry.handle === tab.draft.temporaryHandle);
  const unavailable = !active || !routing.ready || !session || session.terminalTaskBusy || session.terminalRecoveryRequired;
  useEffect(() => {
    if (tab.draft.terminalConfirmed && (!terminal || unavailable)) onUpdate({ terminalConfirmed: false });
  }, [terminal, unavailable, tab.draft.terminalConfirmed, onUpdate]);
  const warnings = [...(tab.execution?.warnings ?? []), ...(tab.response?.warnings ?? [])];
  const knownWarnings = ["SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED", "SSH_TERMINAL_HTTP_REDIRECT_NOT_FOLLOWED"];
  const phaseCode = tab.sendErrorCode;
  const phaseError = session?.terminalRecoveryRequired && tab.sendError && isTerminalApiError(phaseCode) && phaseCode !== "SSH_TERMINAL_RECOVERY_REQUIRED"
    ? t(`network.errors.${phaseCode}`, t("api.route.failed")) : null;
  if (!terminal && !knownWarnings.some((code) => warnings.includes(code)) && tab.sendErrorCode !== "SSH_TERMINAL_CANCELLED") return null;
  return <div className="shrink-0 space-y-1 border-b border-[var(--u-color-border)] px-3 py-2 text-[12px] text-[var(--u-color-text-muted)]">
    {terminal && <>
      <p>{t("api.route.terminalCurlHelp")}</p>
      {phaseError && <p role="alert">{t("api.route.terminalFailure", { message: phaseError })}</p>}
      {session?.terminalRecoveryRequired ? <p role="alert">{t("network.errors.SSH_TERMINAL_RECOVERY_REQUIRED")}</p>
        : session?.terminalTaskBusy && !tab.sending ? <p role="status">{t("network.errors.SSH_TERMINAL_BUSY")}</p> : null}
      <label className="flex items-start gap-2">
        <input type="checkbox" checked={Boolean(tab.draft.terminalConfirmed)} disabled={Boolean(unavailable || tab.sending)}
          onChange={(event) => onUpdate({ terminalConfirmed: event.target.checked })} />
        <span>{t("api.route.terminalConfirmed")}</span>
      </label>
    </>}
    {knownWarnings.filter((code) => warnings.includes(code)).map((code) => <p role="status" key={code}>{t(`network.errors.${code}`)}</p>)}
    {tab.sendErrorCode === "SSH_TERMINAL_CANCELLED" && <p role="status">{t("feedback.apiOutcomeUncertain")}</p>}
  </div>;
}
