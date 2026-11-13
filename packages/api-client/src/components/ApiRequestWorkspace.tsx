import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { Button, SplitPane, useI18n } from "@unfour/ui";

import type { ApiRequestTab } from "../model/request-tabs";
import type { RequestDraft, RequestParamsTab, ResponseTab } from "../model/types";
import { ApiRequestBar } from "./ApiRequestBar";
import { ApiRoutePicker } from "./ApiRoutePicker";
import type { ApiRouteControls } from "../hooks/useApiRoutes";
import { ApiRequestEditor } from "./ApiRequestEditor";
import { ApiResponseViewer } from "./ApiResponseViewer";
import { useApiRequestTabStore } from "../model/api-request-tab-state";

export function ApiRequestWorkspace({
  active = true,
  activeTab,
  workspaceId,
  routing,
  environmentName,
  collectionStatus,
  onRequestNameCommit,
  onRequestTabChange,
  onResponseTabChange,
  onSave,
  onReload,
  onCopyDraft,
  onSend,
  onStop,
  onUpdateDraft,
  urlInputRef,
}: {
  active?: boolean;
  activeTab: ApiRequestTab;
  workspaceId: string;
  routing: ApiRouteControls;
  environmentName?: string;
  collectionStatus: string | null;
  onRequestNameCommit: (tab: ApiRequestTab, name: string) => void;
  onRequestTabChange: (tabId: string, tab: RequestParamsTab) => void;
  onResponseTabChange: (tabId: string, tab: ResponseTab) => void;
  onSave: (tab: ApiRequestTab, name?: string) => void;
  onReload: (tab: ApiRequestTab) => void;
  onCopyDraft: (tab: ApiRequestTab) => void;
  onSend: (tab: ApiRequestTab) => void;
  onStop: (tab: ApiRequestTab) => void;
  onUpdateDraft: (tabId: string, patch: Partial<RequestDraft>) => void;
  urlInputRef: RefObject<HTMLInputElement | null>;
}) {
  const { t } = useI18n();
  const splitHostRef = useRef<HTMLDivElement>(null);
  const [stacked, setStacked] = useState(false);
  useLayoutEffect(() => {
    const host = splitHostRef.current;
    if (!host || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const width = host.getBoundingClientRect().width;
      // Hidden, kept-alive modules report zero; retain their last layout.
      if (width > 0) setStacked(width < 760);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  return (
    <>
      <ApiRequestBar
        key={activeTab.id}
        routeReady={routing.ready}
        routeControl={active && <ApiRoutePicker key={workspaceId} routing={routing} environmentName={environmentName} draft={activeTab.draft.route}
          disabled={activeTab.sending || activeTab.saving} onChange={(route) => onUpdateDraft(activeTab.id, { route })} />}
        onNameEditingChange={(editing) => useApiRequestTabStore.getState().setNameEditing(workspaceId, activeTab.id, editing)}
        onNameCommit={(name) => onRequestNameCommit(activeTab, name)}
        onSave={(name) => onSave(activeTab, name)}
        onSend={() => onSend(activeTab)}
        onStop={() => onStop(activeTab)}
        onUpdate={(patch) => onUpdateDraft(activeTab.id, patch)}
        tab={activeTab}
        urlInputRef={urlInputRef}
      />
      {activeTab.externalChange && (
        <div role="status" className="flex shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-3 py-1 text-[12px] text-[var(--u-color-text-muted)]">
          <span className="flex-1">{t(`api.external.${activeTab.externalChange}`)}</span>
          {activeTab.externalChange === "updated" && <Button size="sm" variant="outline" onClick={() => onReload(activeTab)}>{t("api.external.reload")}</Button>}
          <Button size="sm" variant="ghost" onClick={() => onCopyDraft(activeTab)}>{t("api.external.copyDraft")}</Button>
        </div>
      )}
      {collectionStatus && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] px-2 py-1 text-[12px] text-[var(--u-color-text-muted)]">
          {collectionStatus}
        </div>
      )}
      <div className="flex min-h-0 min-w-0 flex-1" ref={splitHostRef}>
        <SplitPane
          className="min-h-0 flex-1"
          defaultRatio={46}
          minPaneSize={stacked ? 140 : 280}
          orientation={stacked ? "vertical" : "horizontal"}
          resizable
        >
          <ApiRequestEditor
            key={activeTab.id}
            auth={activeTab.draft.auth}
            body={activeTab.draft.body}
            bodyMode={activeTab.draft.bodyMode}
            multipartParts={activeTab.draft.multipartParts}
            onMultipartPartsChange={(multipartParts) => onUpdateDraft(activeTab.id, { multipartParts })}
            formBody={activeTab.draft.formBody}
            headers={activeTab.draft.headers}
            onAuthChange={(auth) => onUpdateDraft(activeTab.id, { auth })}
            onBodyChange={(body) => onUpdateDraft(activeTab.id, { body })}
            onBodyModeChange={(bodyMode) => onUpdateDraft(activeTab.id, { bodyMode })}
            onFormBodyChange={(formBody) => onUpdateDraft(activeTab.id, { formBody })}
            onHeadersChange={(headers) => onUpdateDraft(activeTab.id, { headers })}
            onPostResponseScriptChange={(postResponseScript) =>
              onUpdateDraft(activeTab.id, { postResponseScript })
            }
            onPreRequestScriptChange={(preRequestScript) =>
              onUpdateDraft(activeTab.id, { preRequestScript })
            }
            onQueryChange={(query) => onUpdateDraft(activeTab.id, { query })}
            onRawBodyTypeChange={(rawBodyType) => onUpdateDraft(activeTab.id, { rawBodyType })}
            onTabChange={(tab) => onRequestTabChange(activeTab.id, tab)}
            onTimeoutChange={(timeoutMs) => onUpdateDraft(activeTab.id, { timeoutMs })}
            query={activeTab.draft.query}
            rawBodyType={activeTab.draft.rawBodyType}
            postResponseScript={activeTab.draft.postResponseScript}
            preRequestScript={activeTab.draft.preRequestScript}
            tab={activeTab.requestTab}
            timeoutMs={activeTab.draft.timeoutMs}
          />
          <ApiResponseViewer
            onOpenAuthSettings={() => onRequestTabChange(activeTab.id, "auth")}
            onResponseTabChange={(tab) => onResponseTabChange(activeTab.id, tab)}
            onRetry={() => onSend(activeTab)}
            tab={activeTab}
          />
        </SplitPane>
      </div>
    </>
  );
}
