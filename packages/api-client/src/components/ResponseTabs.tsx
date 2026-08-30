import { responseCookies } from "./response-tab-helpers";
import { AlertCircle, Copy, Send } from "lucide-react";
import { Button, useI18n } from "@unfour/ui";
import { formatByteSize } from "../request-utils";
import {
  deriveTabResponseState,
  type ApiRequestTab,
} from "../model/request-tabs";
import type { ResponseTab } from "../model/types";
import { CompactTabs } from "./RequestParamsTabs";
import { ScriptConsoleView, ScriptTestsView } from "./ScriptResults";
import {
  KeyValueReadout,
  RequestSnapshot,
  ResponseBodyView,
  ResponsePaneState,
  ResponseFailureState,
  PostResponseStatus,
  ResponseStatus,
  ResponseTiming,
  SendingState,
} from "./response-tab-views";

export function ResponseTabs({
  onOpenAuthSettings,
  onResponseTabChange,
  onRetry,
  tab,
}: {
  onOpenAuthSettings: () => void;
  onResponseTabChange: (tab: ResponseTab) => void;
  onRetry: () => void;
  tab: ApiRequestTab;
}) {
  const { t } = useI18n();
  const responseState = deriveTabResponseState(tab);
  const responseSize = tab.response
    ? formatByteSize(new TextEncoder().encode(tab.response.body).length)
    : null;
  const execution = tab.execution;
  const showResponseTabs =
    tab.sending || Boolean(tab.response) || Boolean(tab.lastRequest) || Boolean(execution);
  const canRetry = Boolean(tab.draft.url.trim()) && !tab.sending;
  const scriptTestCount = execution
    ? execution.preRequest.tests.length + execution.postResponse.tests.length
    : 0;
  const consoleCount = execution
    ? execution.preRequest.console.length + execution.postResponse.console.length
    : 0;
  const isScriptTab = tab.responseTab === "tests" || tab.responseTab === "console";

  return (
    <section className="relative flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-surface)]">
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3">
        <span className="text-[11px] font-bold uppercase tracking-[0.07em] text-[var(--u-color-text-muted)]">
          {t("api.response.title")}
        </span>
        <ResponseStatus response={tab.response} state={responseState} />
        <PostResponseStatus result={execution?.postResponse} />
        {tab.response && (
          <div className="ml-auto flex items-center gap-3 font-mono text-[12px] text-[var(--u-color-text-muted)]">
            <span className="font-semibold text-[var(--u-color-text)]">
              {tab.response.durationMs}ms
            </span>
            <span className="font-semibold text-[var(--u-color-text)]">
              {responseSize}
            </span>
            <button
              aria-label={t("api.response.copyBody")}
              className="grid h-7 w-7 place-items-center rounded-[var(--u-radius-md)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]"
              onClick={() => void navigator.clipboard?.writeText(tab.response?.body ?? "")}
              title={t("api.response.copyBody")}
              type="button"
            >
              <Copy size={14} />
            </button>
          </div>
        )}
      </div>
      {showResponseTabs && (
        <CompactTabs
          active={tab.responseTab}
          className="h-[30px] border-b bg-[var(--u-color-surface)]"
          items={[
            { id: "body", label: t("api.response.tabs.body") },
            {
              id: "headers",
              label: t("api.response.tabs.headers"),
              meta: tab.response?.headers.length ?? 0,
            },
            {
              id: "cookies",
              label: t("api.response.tabs.cookies"),
              meta: responseCookies(tab.response).length,
            },
            { id: "timing", label: t("api.response.tabs.timing") },
            { id: "request", label: t("api.response.tabs.request") },
            { id: "tests", label: t("api.response.tabs.tests"), meta: scriptTestCount },
            { id: "console", label: t("api.response.tabs.console"), meta: consoleCount },
          ]}
          onChange={onResponseTabChange}
        />
      )}
      <div className="min-h-0 flex-1 overflow-hidden">
        {responseState === "sending" && <SendingState />}
        {responseState === "idle" && tab.responseTab !== "request" && !isScriptTab && (
          <ResponsePaneState
            description={t("api.response.emptyDescription")}
            icon={<Send size={24} />}
            title={t("api.response.emptyTitle")}
            tone="empty"
          >
            <Button
              disabled={!canRetry}
              onClick={onRetry}
              size="sm"
              type="button"
              variant="outline"
            >
              <Send size={13} />
              {t("api.actions.send")}
              <span className="ml-1 flex items-center gap-0.5">
                <kbd className="rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] px-1 text-[10px]">
                  Ctrl
                </kbd>
                <kbd className="rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] px-1 text-[10px]">
                  Enter
                </kbd>
              </span>
            </Button>
          </ResponsePaneState>
        )}
        {(responseState === "network" ||
          responseState === "timeout" ||
          responseState === "failed") &&
          tab.responseTab !== "request" && !isScriptTab && (
          <ResponseFailureState canRetry={canRetry} onRetry={onRetry} responseState={responseState} sendError={tab.sendError} />
        )}
        {(responseState === "pre-script-error" || responseState === "pre-script-timeout") &&
          !isScriptTab &&
          tab.responseTab !== "request" && (
            <ResponsePaneState
              description={
                execution?.preRequest.error?.message ?? t("api.scripts.unknownError")
              }
              icon={<AlertCircle size={24} />}
              title={
                responseState === "pre-script-timeout"
                  ? t("api.scripts.preTimeoutTitle")
                  : t("api.scripts.preErrorTitle")
              }
              tone="error"
            >
              <Button
                disabled={!canRetry}
                onClick={onRetry}
                size="sm"
                type="button"
                variant="outline"
              >
                {t("api.response.retry")}
              </Button>
            </ResponsePaneState>
          )}
        {responseState !== "sending" && tab.responseTab === "request" && (
          <RequestSnapshot request={tab.lastRequest} />
        )}
        {responseState !== "sending" &&
          !tab.sendError &&
          tab.responseTab === "body" && (
            <ResponseBodyView
              onOpenAuthSettings={onOpenAuthSettings}
              response={tab.response}
            />
          )}
        {!tab.sendError && tab.responseTab === "headers" && (
          <KeyValueReadout
            emptyLabel={t("api.response.noHeaders")}
            items={tab.response?.headers ?? []}
          />
        )}
        {!tab.sendError && tab.responseTab === "cookies" && (
          <KeyValueReadout
            emptyLabel={t("api.response.noCookies")}
            items={responseCookies(tab.response)}
          />
        )}
        {!tab.sendError && tab.responseTab === "timing" && (
          <ResponseTiming response={tab.response} />
        )}
        {execution && tab.responseTab === "tests" && (
          <ScriptTestsView post={execution.postResponse} pre={execution.preRequest} />
        )}
        {execution && tab.responseTab === "console" && (
          <ScriptConsoleView post={execution.postResponse} pre={execution.preRequest} />
        )}
      </div>
    </section>
  );
}
