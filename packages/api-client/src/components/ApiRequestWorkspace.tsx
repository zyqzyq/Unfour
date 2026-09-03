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
  onRequestNameCommit,
  onRequestTabChange,
  onResponseTabChange,
  onSave,
  onSend,
  onStop,
  onUpdateDraft,
  urlInputRef,
}: {
  activeTab: ApiRequestTab;
  collectionStatus: string | null;
  onRequestNameCommit: (tab: ApiRequestTab, name: string) => void;
  onRequestTabChange: (tabId: string, tab: RequestParamsTab) => void;
  onResponseTabChange: (tabId: string, tab: ResponseTab) => void;
  onSave: (tab: ApiRequestTab, name?: string) => void;
  onSend: (tab: ApiRequestTab) => void;
  onStop: (tab: ApiRequestTab) => void;
  onUpdateDraft: (tabId: string, patch: Partial<RequestDraft>) => void;
  urlInputRef: RefObject<HTMLInputElement | null>;
}) {
  return (
    <>
      <ApiRequestBar
        onNameCommit={(name) => onRequestNameCommit(activeTab, name)}
        onSave={(name) => onSave(activeTab, name)}
        onSend={() => onSend(activeTab)}
        onStop={() => onStop(activeTab)}
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
    </>
  );
}
