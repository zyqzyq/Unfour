import { RefreshCw, Terminal } from "lucide-react";
import type {
  SshConnection,
  SshSessionEvent,
  SshSessionSummary,
} from "@unfour/command-client";
import { Button, ConnectionStatus, SplitPane, cn, useI18n } from "@unfour/ui";
import { shouldRenderTerminalPane, terminalSessionStatus, terminalSessionStatusLabel } from "../model/terminal-session-status";
import type { TerminalSplitMode } from "../model/types";
import { TerminalPane } from "./TerminalPane";

export type TerminalPaneModel = {
  connection?: SshConnection | null;
  events: SshSessionEvent[];
  session: SshSessionSummary;
};

export function TerminalSplitView({
  onRetry,
  primary,
  secondary,
  splitMode,
  surfaceActive = true,
}: {
  onRetry: (connectionId: string) => void;
  primary: TerminalPaneModel;
  secondary: TerminalPaneModel | null;
  splitMode: TerminalSplitMode;
  surfaceActive?: boolean;
}) {
  const { t } = useI18n();
  const split = splitMode !== "single";

  const primaryPane = (
    <PaneShell
      active={surfaceActive}
      model={primary}
      onRetry={onRetry}
      paintActive={surfaceActive}
    />
  );

  if (!split) {
    return primaryPane;
  }

  return (
    <SplitPane
      className="min-h-0 flex-1 bg-[var(--u-color-terminal-bg)]"
      defaultRatio={50}
      minPaneSize={180}
      orientation={splitMode === "horizontal" ? "vertical" : "horizontal"}
      resizable
    >
      {primaryPane}
      <div className={cn("flex min-h-0 min-w-0 flex-1 flex-col")}>
        {secondary ? (
          <PaneShell
            model={secondary}
            onRetry={onRetry}
            paintActive={surfaceActive}
            readOnly
          />
        ) : (
          <div className="flex min-h-0 flex-1 items-center justify-center bg-[var(--u-color-terminal-bg)] p-3 font-mono text-[12px] text-[var(--u-color-terminal-muted)]">
            {t("ssh.pane.secondaryPlaceholder")}
          </div>
        )}
      </div>
    </SplitPane>
  );
}

function PaneShell({
  active,
  model,
  onRetry,
  paintActive = true,
  readOnly,
}: {
  active?: boolean;
  model: TerminalPaneModel;
  onRetry: (connectionId: string) => void;
  /** False while Connections is hidden under Tasks. */
  paintActive?: boolean;
  readOnly?: boolean;
}) {
  const { t } = useI18n();
  const { events, session } = model;
  const showFailedPlaceholder =
    session.status === "failed" &&
    !shouldRenderTerminalPane(session, events.length);

  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 flex-1 flex-col border-t-2 bg-[var(--u-color-terminal-bg)]",
        active ? "border-t-[var(--u-color-terminal-cursor)]" : "border-t-[var(--u-color-terminal-border)]",
      )}
    >
      <div className="flex h-10 shrink-0 items-center gap-2.5 border-b border-[var(--u-color-terminal-border)] px-4 text-[11px] text-[var(--u-color-terminal-muted)]">
        <Terminal aria-hidden="true" size={14} className="shrink-0 text-[var(--u-color-terminal-cursor)]" />
        <span className="min-w-0 flex-1 truncate font-mono" title={`${session.username}@${session.host}`}>{model.connection?.name ?? `${session.username}@${session.host}`}</span>
        <span className="flex shrink-0 items-center gap-1.5"><ConnectionStatus dotOnly label={terminalSessionStatusLabel(session, t)} status={terminalSessionStatus(session)} variant="dot" /><span>{terminalSessionStatusLabel(session, t)}</span></span>
      </div>
      {showFailedPlaceholder ? (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-[var(--u-color-terminal-bg)] p-6 text-center">
          <div className="space-y-1">
            <div className="text-[13px] font-semibold text-[var(--u-color-terminal-text)]">
              {t("ssh.pane.failedTitle")}
            </div>
            <div className="text-[12px] text-[var(--u-color-terminal-muted)]">
              {t("ssh.pane.failedDetail", { host: session.host })}
            </div>
          </div>
          <Button
            onClick={() => onRetry(session.connectionId)}
            size="sm"
            type="button"
            variant="outline"
          >
            <RefreshCw size={14} />
            {t("ssh.pane.retry")}
          </Button>
        </div>
      ) : (
        <TerminalPane
          active={active}
          events={events}
          inputDisabled={session.status !== "connected"}
          paintActive={paintActive}
          readOnly={readOnly}
          session={session}
        />
      )}
    </div>
  );
}
