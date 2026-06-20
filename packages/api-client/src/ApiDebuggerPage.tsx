import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button, EmptyState, SplitPane, useI18n } from "@unfour/ui";
import { useApiRequestTabs } from "./hooks/useApiRequestTabs";
import { useApiCollections } from "./hooks/useApiCollections";
import {
  getTabSaveState,
  requestTabTitle,
  type ApiRequestTab,
} from "./model/request-tabs";
import type { ApiOpenIntent } from "./model/types";
import { formatError } from "./model/api-request-state";
import { parseCollectionImport, parseKeyValues } from "./request-utils";
import { ApiRequestTabs } from "./components/ApiRequestTabs";
import { ApiRequestBar } from "./components/ApiRequestBar";
import { ApiRequestEditor } from "./components/ApiRequestEditor";
import { ApiResponseViewer } from "./components/ApiResponseViewer";
import { ApiSaveDialog, type SaveIdentity } from "./components/ApiSaveDialog";
import { ApiCloseRequestDialog } from "./components/ApiCloseRequestDialog";
import { ApiClientSidebar } from "./components/ApiClientSidebar";

export function ApiDebuggerPage({
  onActiveSavedRequestChange,
  onShellSidebarChange,
  openIntent,
  workspaceId,
}: {
  onActiveSavedRequestChange?: (requestId: string | null) => void;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  openIntent: ApiOpenIntent | null;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const {
    activateEnvironment,
    activeEnvironment,
    activeTab,
    closeTab,
    closeTabs,
    collectionStatus,
    deleteMutation,
    duplicateMutation,
    importCollectionMutation,
    importInputRef,
    newRequest,
    openHistory,
    openSaved,
    saveTab,
    savedRequests,
    selectTab,
    sendTab,
    setCollectionStatus,
    setRequestTab,
    setResponseTab,
    state,
    updateDraft,
  } = useApiRequestTabs(workspaceId);
  const { collections, createMut: createCollectionMut } =
    useApiCollections(workspaceId);
  const [saveDialogTabId, setSaveDialogTabId] = useState<string | null>(null);
  const [closeDialogTabId, setCloseDialogTabId] = useState<string | null>(null);
  const closeAfterSaveRef = useRef<string | null>(null);
  const pendingCloseQueueRef = useRef<string[]>([]);
  const urlInputRef = useRef<HTMLInputElement>(null);
  const pendingIntentAction = useRef<{
    action: "save" | "send";
    tabId: string;
  } | null>(null);

  const requestSave = useCallback(
    (tab: ApiRequestTab) => {
      if (tab.savedRequestId) {
        void saveTab(tab);
      } else {
        setSaveDialogTabId(tab.id);
      }
    },
    [saveTab],
  );

  useEffect(
    () => onActiveSavedRequestChange?.(activeTab?.savedRequestId ?? null),
    [activeTab?.savedRequestId, onActiveSavedRequestChange],
  );

  useEffect(() => {
    if (!openIntent) {
      return;
    }
    if (openIntent.kind === "new") {
      newRequest();
      return;
    }
    if (openIntent.kind === "saved") {
      if (openIntent.action === "send") {
        pendingIntentAction.current = {
          action: "send",
          tabId: `saved:${openIntent.requestId}`,
        };
      }
      openSaved(openIntent.requestId);
      return;
    }
    if (openIntent.action === "save") {
      pendingIntentAction.current = {
        action: "save",
        tabId: `history:${openIntent.historyId}`,
      };
    }
    void openHistory(openIntent.historyId);
  }, [newRequest, openHistory, openIntent, openSaved]);

  useEffect(() => {
    const pending = pendingIntentAction.current;
    if (!pending || activeTab?.id !== pending.tabId) {
      return;
    }
    pendingIntentAction.current = null;
    if (pending.action === "send") {
      sendTab(activeTab);
    } else {
      setSaveDialogTabId(activeTab.id);
    }
  }, [activeTab, sendTab]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.ctrlKey || event.metaKey) || !activeTab) {
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        sendTab(activeTab);
      }
      if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        requestSave(activeTab);
      }
      if (event.key.toLowerCase() === "l") {
        event.preventDefault();
        urlInputRef.current?.focus();
        urlInputRef.current?.select();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeTab, requestSave, sendTab]);

  const saveDialogTab =
    state.tabs.find((tab) => tab.id === saveDialogTabId) ?? null;
  const closeDialogTab =
    state.tabs.find((tab) => tab.id === closeDialogTabId) ?? null;

  function continuePendingCloseQueue() {
    const immediateIds: string[] = [];
    while (pendingCloseQueueRef.current.length) {
      const tabId = pendingCloseQueueRef.current.shift();
      const tab = state.tabs.find((item) => item.id === tabId);
      if (!tab) {
        continue;
      }
      if (getTabSaveState(tab) === "saved") {
        immediateIds.push(tab.id);
        continue;
      }
      if (immediateIds.length) {
        closeTabs(immediateIds);
      }
      setCloseDialogTabId(tab.id);
      return;
    }
    if (immediateIds.length) {
      closeTabs(immediateIds);
    }
  }

  function requestClose(tab: ApiRequestTab) {
    requestCloseMany([tab]);
  }

  function requestCloseMany(tabsToClose: ApiRequestTab[]) {
    pendingCloseQueueRef.current = tabsToClose.map((tab) => tab.id);
    continuePendingCloseQueue();
  }

  function requestCloseSavedTabs() {
    requestCloseMany(
      state.tabs.filter((tab) => getTabSaveState(tab) === "saved"),
    );
  }

  function requestCloseTabsLeftOf(anchor: ApiRequestTab) {
    const index = state.tabs.findIndex((tab) => tab.id === anchor.id);
    if (index > 0) {
      requestCloseMany(state.tabs.slice(0, index));
    }
  }

  function requestCloseTabsRightOf(anchor: ApiRequestTab) {
    const index = state.tabs.findIndex((tab) => tab.id === anchor.id);
    if (index >= 0) {
      requestCloseMany(state.tabs.slice(index + 1));
    }
  }

  async function saveWithIdentity(identity: SaveIdentity) {
    if (!saveDialogTab) {
      return;
    }
    const originalId = saveDialogTab.id;
    let collectionId = identity.collectionId;
    if (identity.createCollectionName) {
      try {
        const created = await createCollectionMut.mutateAsync(
          identity.createCollectionName,
        );
        collectionId = created.id;
      } catch {
        return;
      }
    }
    const savedRequestId = await saveTab(saveDialogTab, {
      collectionId,
      folderPath: identity.folderPath,
      name: identity.name,
    });
    if (savedRequestId) {
      setSaveDialogTabId(null);
      if (closeAfterSaveRef.current === originalId) {
        closeAfterSaveRef.current = null;
        closeTab(`saved:${savedRequestId}`);
        continuePendingCloseQueue();
      }
    }
  }

  async function saveThenClose(tab: ApiRequestTab) {
    setCloseDialogTabId(null);
    if (!tab.savedRequestId) {
      closeAfterSaveRef.current = tab.id;
      setSaveDialogTabId(tab.id);
      return;
    }
    if (await saveTab(tab)) {
      closeTab(tab.id);
      continuePendingCloseQueue();
    }
  }

  const handleSidebarIntent = useCallback(
    (intent: ApiOpenIntent) => {
      if (intent.kind === "new") {
        newRequest();
        return;
      }
      if (intent.kind === "saved") {
        if (intent.action === "send") {
          pendingIntentAction.current = {
            action: "send",
            tabId: `saved:${intent.requestId}`,
          };
        }
        openSaved(intent.requestId);
        return;
      }
      if (intent.action === "save") {
        pendingIntentAction.current = {
          action: "save",
          tabId: `history:${intent.historyId}`,
        };
      }
      void openHistory(intent.historyId);
    },
    [newRequest, openHistory, openSaved],
  );

  const usesShellSidebar = Boolean(onShellSidebarChange);
  const sidebar = useMemo(
    () => (
      <ApiClientSidebar
        onNewRequest={newRequest}
        onOpenIntent={handleSidebarIntent}
        selectedId={activeTab?.savedRequestId ?? null}
        shellSlot={usesShellSidebar}
        workspaceId={workspaceId}
      />
    ),
    [
      activeTab?.savedRequestId,
      handleSidebarIntent,
      newRequest,
      usesShellSidebar,
      workspaceId,
    ],
  );

  useEffect(() => {
    if (!onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(sidebar);
    return () => onShellSidebarChange(null);
  }, [onShellSidebarChange, sidebar]);

  function exportCollection() {
    const payload = {
      version: 1,
      exportedAt: new Date().toISOString(),
      workspaceId,
      savedRequests: savedRequests.map((item) => ({
        name: item.name,
        folderPath: item.folderPath,
        method: item.method,
        url: item.url,
        headers: parseKeyValues(item.headersJson),
        query: parseKeyValues(item.queryJson),
        body: item.body,
        bodyKind: item.bodyKind,
      })),
    };
    downloadJson(
      `unfour-api-collection-${new Date().toISOString().slice(0, 10)}.json`,
      payload,
    );
    setCollectionStatus(t("api.import.exported", { count: payload.savedRequests.length }));
  }

  async function importCollection(file: File | undefined) {
    if (!file) {
      return;
    }
    try {
      const requests = parseCollectionImport(JSON.parse(await file.text()), workspaceId);
      if (!requests.length) {
        setCollectionStatus(t("api.import.empty"));
        return;
      }
      importCollectionMutation.mutate(requests);
    } catch (error) {
      setCollectionStatus(formatError(error));
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--u-color-bg)]">
      <input
        accept="application/json"
        className="sr-only"
        onChange={(event) => {
          void importCollection(event.target.files?.[0]);
          event.target.value = "";
        }}
        ref={importInputRef}
        type="file"
      />
      <div className="flex min-h-0 flex-1">
        {!usesShellSidebar && sidebar}
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <ApiRequestTabs
            activeId={state.activeTabId}
            onClose={requestClose}
            onCloseAll={() => requestCloseMany(state.tabs)}
            onCloseLeft={requestCloseTabsLeftOf}
            onCloseRight={requestCloseTabsRightOf}
            onCloseSaved={requestCloseSavedTabs}
            onNew={newRequest}
            onSelect={selectTab}
            tabs={state.tabs}
          />
          {!activeTab ? (
            <EmptyState className="m-3 flex-1">
              <div className="space-y-2">
                <div>{t("api.empty.noRequestOpen")}</div>
                <Button onClick={newRequest} type="button">
                  {t("common.actions.newRequest")}
                </Button>
              </div>
            </EmptyState>
          ) : (
            <>
              <ApiRequestBar
                activeEnvironmentId={activeEnvironment?.id ?? null}
                onDelete={() =>
                  activeTab.savedRequestId &&
                  deleteMutation.mutate(activeTab.savedRequestId)
                }
                onDuplicate={() =>
                  activeTab.savedRequestId &&
                  duplicateMutation.mutate(activeTab.savedRequestId)
                }
                onExport={exportCollection}
                onImport={() => importInputRef.current?.click()}
                onSave={() => requestSave(activeTab)}
                onSelectEnvironment={activateEnvironment}
                onSend={() => sendTab(activeTab)}
                onUpdate={(patch) => updateDraft(activeTab.id, patch)}
                tab={activeTab}
                urlInputRef={urlInputRef}
                workspaceId={workspaceId}
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
                  onAuthChange={(auth) => updateDraft(activeTab.id, { auth })}
                  onBodyChange={(body) => updateDraft(activeTab.id, { body })}
                  onBodyModeChange={(bodyMode) =>
                    updateDraft(activeTab.id, { bodyMode })
                  }
                  onFormBodyChange={(formBody) =>
                    updateDraft(activeTab.id, { formBody })
                  }
                  onHeadersChange={(headers) => updateDraft(activeTab.id, { headers })}
                  onQueryChange={(query) => updateDraft(activeTab.id, { query })}
                  onRawBodyTypeChange={(rawBodyType) =>
                    updateDraft(activeTab.id, { rawBodyType })
                  }
                  onTabChange={(tab) => setRequestTab(activeTab.id, tab)}
                  query={activeTab.draft.query}
                  rawBodyType={activeTab.draft.rawBodyType}
                  tab={activeTab.requestTab}
                />
                <ApiResponseViewer
                  onOpenAuthSettings={() => setRequestTab(activeTab.id, "auth")}
                  onResponseTabChange={(tab) => setResponseTab(activeTab.id, tab)}
                  onRetry={() => sendTab(activeTab)}
                  tab={activeTab}
                />
              </SplitPane>
            </>
          )}
        </div>
      </div>
      {saveDialogTab && (
        <ApiSaveDialog
          collections={collections}
          defaultCollectionId={saveDialogTab.draft.collectionId}
          defaultFolder={saveDialogTab.draft.folderPath}
          defaultName={saveDialogTab.draft.name}
          key={saveDialogTab.id}
          savedRequests={savedRequests}
          onCancel={() => {
            closeAfterSaveRef.current = null;
            pendingCloseQueueRef.current = [];
            setSaveDialogTabId(null);
          }}
          onSave={(identity) => void saveWithIdentity(identity)}
          open
          saving={saveDialogTab.saving}
        />
      )}
      <ApiCloseRequestDialog
        onCancel={() => {
          pendingCloseQueueRef.current = [];
          setCloseDialogTabId(null);
        }}
        onDiscard={() => {
          if (closeDialogTab) {
            closeTab(closeDialogTab.id);
          }
          setCloseDialogTabId(null);
          continuePendingCloseQueue();
        }}
        onSave={() => closeDialogTab && void saveThenClose(closeDialogTab)}
        open={Boolean(closeDialogTab)}
        title={closeDialogTab ? requestTabTitle(closeDialogTab) : ""}
      />
    </div>
  );
}

function downloadJson(filename: string, value: unknown) {
  const href = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], {
      type: "application/json;charset=utf-8",
    }),
  );
  const link = document.createElement("a");
  link.href = href;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(href);
}
