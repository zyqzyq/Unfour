import type { RefObject } from "react";
import { SplitPane } from "@unfour/ui";

import type { ApiRequestTab } from "../model/request-tabs";
import type { RequestDraft, RequestParamsTab, ResponseTab } from "../model/types";
import { ApiRequestBar } from "./ApiRequestBar";
import { ApiRequestEditor } from "./ApiRequestEditor";
import { ApiResponseViewer } from "./ApiResponseViewer";

export function ApiRequestWorkspace({
  activeTab,
  collectionStatus,
  onRequestTabChange,
  onResponseTabChange,
  onSave,
  onSend,
  onUpdateDraft,
  urlInputRef,
}: {
  activeTab: ApiRequestTab;
  collectionStatus: string | null;
  onRequestTabChange: (tabId: string, tab: RequestParamsTab) => void;
  onResponseTabChange: (tabId: string, tab: ResponseTab) => void;
  onSave: (tab: ApiRequestTab) => void;
  onSend: (tab: ApiRequestTab) => void;
  onUpdateDraft: (tabId: string, patch: Partial<RequestDraft>) => void;
  urlInputRef: RefObject<HTMLInputElement | null>;
}) {
  return (
    <>
      <ApiRequestBar
        onSave={() => onSave(activeTab)}
        onSend={() => onSend(activeTab)}
        onUpdate={(patch) => onUpdateDraft(activeTab.id, patch)}
        tab={activeTab}
        urlInputRef={urlInputRef}
      />
      {collectionStatus && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] px-2 py-1 text-[12px] text-[var(--u-color-text-muted)]">
          {collectionStatus}
        </div>
      )}
      <SplitPane
        className="min-h-0 flex-1"
        defaultRatio={46}
        minPaneSize={280}
        orientation="horizontal"
        resizable
      >
        <ApiRequestEditor
          auth={activeTab.draft.auth}
          body={activeTab.draft.body}
          bodyMode={activeTab.draft.bodyMode}
          formBody={activeTab.draft.formBody}
          headers={activeTab.draft.headers}
          onAuthChange={(auth) => onUpdateDraft(activeTab.id, { auth })}
          onBodyChange={(body) => onUpdateDraft(activeTab.id, { body })}
          onBodyModeChange={(bodyMode) => onUpdateDraft(activeTab.id, { bodyMode })}
          onFormBodyChange={(formBody) => onUpdateDraft(activeTab.id, { formBody })}
          onHeadersChange={(headers) => onUpdateDraft(activeTab.id, { headers })}
          onQueryChange={(query) => onUpdateDraft(activeTab.id, { query })}
          onRawBodyTypeChange={(rawBodyType) => onUpdateDraft(activeTab.id, { rawBodyType })}
          onTabChange={(tab) => onRequestTabChange(activeTab.id, tab)}
          query={activeTab.draft.query}
          rawBodyType={activeTab.draft.rawBodyType}
          tab={activeTab.requestTab}
        />
        <ApiResponseViewer
          onOpenAuthSettings={() => onRequestTabChange(activeTab.id, "auth")}
          onResponseTabChange={(tab) => onResponseTabChange(activeTab.id, tab)}
          onRetry={() => onSend(activeTab)}
          tab={activeTab}
        />
      </SplitPane>
    </>
  );
}
