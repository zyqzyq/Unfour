import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button, EmptyState, useI18n } from "@unfour/ui";
import { useApiRequestTabs } from "./hooks/useApiRequestTabs";
import { useApiCollections } from "./hooks/useApiCollections";
import { useApiCollectionFolders } from "./hooks/useApiCollectionFolders";
import {
  getTabSaveState,
  type ApiRequestTab,
} from "./model/request-tabs";
import type { ApiOpenIntent } from "./model/types";
import { ApiRequestTabs } from "./components/ApiRequestTabs";
import type { SaveIdentity } from "./components/ApiSaveDialog";
import { findDuplicateRequestName } from "./request-utils";
import { ApiClientSidebar } from "./components/ApiClientSidebar";
import { ApiClientDialogs } from "./components/ApiClientDialogs";
import { ApiRequestWorkspace } from "./components/ApiRequestWorkspace";

export function ApiClientPage({
  active = true,
  onActiveSavedRequestChange,
  onShellSidebarChange,
  openIntent,
  workspaceId,
}: {
  active?: boolean;
  onActiveSavedRequestChange?: (requestId: string | null) => void;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  openIntent: ApiOpenIntent | null;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const {
    activeTab,
    closeTab,
    closeTabs,
    collectionStatus,
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
  const { createFolderMut, folders } = useApiCollectionFolders(workspaceId);
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
        const duplicate = findDuplicateRequestName(
          savedRequests,
          tab.draft.name,
          tab.draft.collectionId,
          tab.draft.parentFolderId,
          tab.savedRequestId,
        );
        if (duplicate) {
          setCollectionStatus(t("api.save.duplicateName", { name: tab.draft.name }));
          return;
        }
        void saveTab(tab);
      } else {
        setSaveDialogTabId(tab.id);
      }
    },
    [saveTab, savedRequests, setCollectionStatus, t],
  );

  const handleNewRequest = useCallback(() => {
    newRequest();
  }, [newRequest]);

  const handleSelectTab = useCallback((tabId: string) => {
    selectTab(tabId);
  }, [selectTab]);

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
    if (!active) return;
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
  }, [active, activeTab, requestSave, sendTab]);

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
    let parentFolderId = identity.parentFolderId;
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
    if (identity.newFolderName) {
      try {
        if (!collectionId) {
          const created = await createCollectionMut.mutateAsync(
            t("api.collection.defaultCollection"),
          );
          collectionId = created.id;
        }
        const folder = await createFolderMut.mutateAsync({
          collectionId,
          name: identity.newFolderName,
          parentFolderId,
        });
        parentFolderId = folder.id;
      } catch {
        return;
      }
    }
    const savedRequestId = await saveTab(saveDialogTab, {
      collectionId,
      name: identity.name,
      parentFolderId,
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
    const duplicate = findDuplicateRequestName(
      savedRequests,
      tab.draft.name,
      tab.draft.collectionId,
      tab.draft.parentFolderId,
      tab.savedRequestId,
    );
    if (duplicate) {
      setCollectionStatus(t("api.save.duplicateName", { name: tab.draft.name }));
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
        onNewRequest={handleNewRequest}
        onOpenIntent={handleSidebarIntent}
        selectedId={activeTab?.savedRequestId ?? null}
        shellSlot={usesShellSidebar}
        workspaceId={workspaceId}
      />
    ),
    [
      activeTab?.savedRequestId,
      handleNewRequest,
      handleSidebarIntent,
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


  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--u-color-bg)]">
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
            onNew={handleNewRequest}
            onSelect={handleSelectTab}
            tabs={state.tabs}
          />
          {!activeTab ? (
            <EmptyState className="m-3 flex-1">
              <div className="space-y-2">
                <div>{t("api.empty.noRequestOpen")}</div>
                <Button onClick={handleNewRequest} type="button">
                  {t("common.actions.newRequest")}
                </Button>
              </div>
            </EmptyState>
          ) : (
            <ApiRequestWorkspace
              activeTab={activeTab}
              collectionStatus={collectionStatus}
              onRequestTabChange={setRequestTab}
              onResponseTabChange={setResponseTab}
              onSave={requestSave}
              onSend={sendTab}
              onUpdateDraft={updateDraft}
              urlInputRef={urlInputRef}
            />
          )}
        </div>
      </div>
      <ApiClientDialogs
        closeDialogTab={closeDialogTab}
        collections={collections}
        folders={folders}
        onCancelClose={() => { pendingCloseQueueRef.current = []; setCloseDialogTabId(null); }}
        onCancelSave={() => { closeAfterSaveRef.current = null; pendingCloseQueueRef.current = []; setSaveDialogTabId(null); }}
        onDiscardClose={() => { if (closeDialogTab) closeTab(closeDialogTab.id); setCloseDialogTabId(null); continuePendingCloseQueue(); }}
        onSaveClose={() => closeDialogTab && void saveThenClose(closeDialogTab)}
        onSaveIdentity={(identity) => void saveWithIdentity(identity)}
        savedRequests={savedRequests}
        saveDialogTab={saveDialogTab}
      />
    </div>
  );
}
