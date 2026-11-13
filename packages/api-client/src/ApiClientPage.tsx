import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button, EmptyState, useI18n } from "@unfour/ui";
import { formatError } from "./model/api-request-state";
import { historyReplayError } from "./model/history-replay";
import { useApiRequestTabs } from "./hooks/useApiRequestTabs";
import { useApiCollections } from "./hooks/useApiCollections";
import { useApiCollectionFolders } from "./hooks/useApiCollectionFolders";
import {
  getTabSaveState,
  requestTabNeedsCloseConfirmation,
  type ApiRequestTab,
} from "./model/request-tabs";
import type { ApiOpenIntent } from "./model/types";
import { ApiRequestTabs } from "./components/ApiRequestTabs";
import type { SaveIdentity } from "./components/ApiSaveDialog";
import { findDuplicateRequestName } from "./request-utils";
import { ApiClientSidebar } from "./components/ApiClientSidebar";
import { ApiClientDialogs } from "./components/ApiClientDialogs";
import { ApiRequestWorkspace } from "./components/ApiRequestWorkspace";
import { ApiCurlImport } from "./components/ApiCurlImport";

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
    activeEnvironment,
    routing,
    activeTab,
    closeTab,
    closeTabs,
    collectionStatus,
    cancelTab,
    newRequest,
    openHistory,
    openSaved,
    reloadTab,
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
  const [saveDialogError, setSaveDialogError] = useState<string | null>(null);
  const [saveDialogPending, setSaveDialogPending] = useState(false);
  const saveDialogPendingRef = useRef(false);
  const [closeDialogTabId, setCloseDialogTabId] = useState<string | null>(null);
  const closeAfterSaveRef = useRef<string | null>(null);
  const pendingCloseQueueRef = useRef<string[]>([]);
  const urlInputRef = useRef<HTMLInputElement>(null);
  const pageRef = useRef<HTMLDivElement>(null);
  const consumedOpenIntent = useRef<{ workspaceId: string; nonce: number } | null>(null);
  const pendingHistorySave = useRef<string | null>(null);
  const pendingSend = useRef<{
    tab: ApiRequestTab;
    selected: boolean;
    workspaceId: string;
    environmentId: string | null | undefined;
  } | null>(null);
  const environmentId = routing.environmentStatus === "success" ? activeEnvironment?.id ?? null : undefined;

  const requestSave = useCallback(
    (tab: ApiRequestTab, nameOverride?: string) => {
      const effectiveTab =
        nameOverride === undefined
          ? tab
          : { ...tab, draft: { ...tab.draft, name: nameOverride } };
      if (nameOverride !== undefined) {
        updateDraft(tab.id, { name: nameOverride });
      }

      if (effectiveTab.savedRequestId) {
        const duplicate = findDuplicateRequestName(
          savedRequests,
          effectiveTab.draft.name,
          effectiveTab.draft.collectionId,
          effectiveTab.draft.parentFolderId,
          effectiveTab.savedRequestId,
        );
        if (duplicate) {
          setCollectionStatus(
            t("api.save.duplicateName", { name: effectiveTab.draft.name }),
          );
          return;
        }
        void saveTab(effectiveTab);
      } else {
        setSaveDialogError(null);
        setSaveDialogTabId(tab.id);
      }
    },
    [saveTab, savedRequests, setCollectionStatus, t, updateDraft],
  );

  const handleRequestNameCommit = useCallback(
    (tab: ApiRequestTab, name: string) => {
      const nextName = name.trim();
      if (!nextName && tab.savedRequestId) {
        return;
      }
      updateDraft(tab.id, { name: nextName });
    },
    [updateDraft],
  );

  const handleNewRequest = useCallback(() => {
    pendingSend.current = null;
    newRequest();
  }, [newRequest]);

  const handleSelectTab = useCallback((tabId: string) => {
    if (pendingSend.current?.tab.id !== tabId) pendingSend.current = null;
    selectTab(tabId);
  }, [selectTab]);

  useEffect(
    () => onActiveSavedRequestChange?.(activeTab?.savedRequestId ?? null),
    [activeTab?.savedRequestId, onActiveSavedRequestChange],
  );

  const handleOpenIntent = useCallback((intent: ApiOpenIntent) => {
    pendingHistorySave.current = null;
    pendingSend.current = null;
    if (intent.kind === "new") newRequest();
    else if (intent.kind === "saved") {
      const tab = openSaved(intent.requestId);
      if (tab && intent.action === "send") {
        pendingSend.current = { tab, workspaceId, environmentId, selected: false };
      }
    } else {
      if (intent.action === "save") pendingHistorySave.current = `history:${intent.historyId}`;
      void openHistory(intent.historyId);
    }
  }, [environmentId, newRequest, openHistory, openSaved, workspaceId]);

  useEffect(() => {
    if (!openIntent) return;
    if (consumedOpenIntent.current?.workspaceId === workspaceId && consumedOpenIntent.current.nonce === openIntent.nonce) return;
    // Wait for saved data, then consume the action once even if that query refreshes.
    if (openIntent.kind === "saved" && !savedRequests.some((item) => item.id === openIntent.requestId)) return;
    consumedOpenIntent.current = { workspaceId, nonce: openIntent.nonce };
    handleOpenIntent(openIntent);
  }, [handleOpenIntent, openIntent, savedRequests, workspaceId]);

  useEffect(() => {
    if (activeTab && pendingHistorySave.current === activeTab.id) {
      pendingHistorySave.current = null;
      setSaveDialogTabId(activeTab.id);
    }
  }, [activeTab, openIntent, state]);

  // openSaved publishes a new tab slice even when its target is already active.
  useEffect(() => {
    const pending = pendingSend.current;
    if (!pending) return;
    if (!active || workspaceId !== pending.workspaceId) {
      pendingSend.current = null;
      return;
    }
    // openSaved can select the target during the effect above. Wait for that
    // selection to render, but never wait for a previously selected tab to return.
    if (activeTab?.id !== pending.tab.id) {
      if (pending.selected) pendingSend.current = null;
      return;
    }
    pending.selected = true;
    if (pending.environmentId === undefined) pending.environmentId = environmentId;
    const sameRequest = activeTab.draft === pending.tab.draft && pending.environmentId === environmentId;
    if (sameRequest && routing.pending) return;
    pendingSend.current = null;
    if (sameRequest) sendTab(activeTab);
  }, [active, activeTab, environmentId, openIntent, routing.pending, sendTab, state, workspaceId]);

  useEffect(() => {
    if (!active) return;
    function onKeyDown(event: KeyboardEvent) {
      // Bubble-phase listener: editors and inputs get first refusal. Portalled
      // interaction layers also own shortcuts when focus has not moved yet.
      if (
        !(event.ctrlKey || event.metaKey) || !activeTab ||
        event.defaultPrevented || event.isComposing || event.keyCode === 229 ||
        event.repeat || event.altKey || event.shiftKey ||
        saveDialogTabId || closeDialogTabId ||
        document.querySelector(
          '[role="dialog"]:not([data-state="closed"]), [role="alertdialog"]:not([data-state="closed"]), [role="menu"][data-state="open"], [role="listbox"][data-state="open"]',
        )
      ) {
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
  }, [active, activeTab, closeDialogTabId, requestSave, saveDialogTabId, sendTab]);

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
      if (!requestTabNeedsCloseConfirmation(tab)) {
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
    if (tabsToClose.some((tab) => tab.id === pendingSend.current?.tab.id)) pendingSend.current = null;
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
    if (!saveDialogTab || saveDialogPendingRef.current) {
      return;
    }
    const replayError = historyReplayError(saveDialogTab, t);
    if (replayError) {
      setSaveDialogError(replayError);
      return;
    }
    saveDialogPendingRef.current = true;
    setSaveDialogPending(true);
    const originalId = saveDialogTab.id;
    setSaveDialogError(null);
    let collectionId = identity.collectionId;
    let parentFolderId = identity.parentFolderId;
    try {
      if (identity.createCollectionName) {
        try {
          const created = await createCollectionMut.mutateAsync(
            identity.createCollectionName,
          );
          collectionId = created.id;
        } catch (error) {
          setSaveDialogError(formatError(error));
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
        } catch (error) {
          setSaveDialogError(formatError(error));
          return;
        }
      }
      const savedRequestId = await saveTab(saveDialogTab, {
        collectionId,
        name: identity.name,
        parentFolderId,
      });
      if (savedRequestId) {
        setSaveDialogError(null);
        setSaveDialogTabId(null);
        if (closeAfterSaveRef.current === originalId) {
          closeAfterSaveRef.current = null;
          closeTab(`saved:${savedRequestId}`);
          continuePendingCloseQueue();
        }
      }
    } finally {
      saveDialogPendingRef.current = false;
      setSaveDialogPending(false);
    }
  }

  async function saveThenClose(tab: ApiRequestTab) {
    setCloseDialogTabId(null);
    if (!tab.savedRequestId) {
      closeAfterSaveRef.current = tab.id;
      setSaveDialogError(null);
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

  const usesShellSidebar = Boolean(onShellSidebarChange);
  const sidebar = useMemo(
    () => (
      <ApiClientSidebar
        onNewRequest={handleNewRequest}
        onOpenIntent={handleOpenIntent}
        selectedId={activeTab?.savedRequestId ?? null}
        shellSlot={usesShellSidebar}
        workspaceId={workspaceId}
      />
    ),
    [
      activeTab?.savedRequestId,
      handleNewRequest,
      handleOpenIntent,
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
    <div ref={pageRef} className="flex h-full min-h-0 flex-col bg-[var(--u-color-bg)]">
      <div className="flex min-h-0 flex-1">
        {!usesShellSidebar && sidebar}
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <ApiRequestTabs
            endControl={<ApiCurlImport key={workspaceId} onImport={newRequest} />}
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
              active={active}
              routing={routing}
              environmentName={activeEnvironment?.name}
              workspaceId={workspaceId}
              activeTab={activeTab}
              collectionStatus={collectionStatus}
              onRequestNameCommit={handleRequestNameCommit}
              onRequestTabChange={setRequestTab}
              onResponseTabChange={setResponseTab}
              onSave={requestSave}
              onReload={(tab) => void reloadTab(tab)}
              onCopyDraft={(tab) => newRequest({ ...tab.draft, collectionId: null, parentFolderId: null })}
              onSend={sendTab}
              onStop={(tab) => void cancelTab(tab)}
              onUpdateDraft={updateDraft}
              urlInputRef={urlInputRef}
            />
          )}
        </div>
      </div>
      <ApiClientDialogs
        onRestoreFocus={() => pageRef.current?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus()}
        closeDialogTab={closeDialogTab}
        collections={collections}
        folders={folders}
        onCancelClose={() => { pendingCloseQueueRef.current = []; setCloseDialogTabId(null); }}
        onCancelSave={() => {
          if (saveDialogPendingRef.current) {
            return;
          }
          closeAfterSaveRef.current = null;
          pendingCloseQueueRef.current = [];
          setSaveDialogError(null);
          setSaveDialogTabId(null);
        }}
        saveDialogError={saveDialogError ?? saveDialogTab?.saveError ?? null}
        saveDialogPending={saveDialogPending}
        onDiscardClose={() => { if (closeDialogTab) closeTab(closeDialogTab.id); setCloseDialogTabId(null); continuePendingCloseQueue(); }}
        onSaveClose={() => closeDialogTab && void saveThenClose(closeDialogTab)}
        onSaveIdentity={(identity) => void saveWithIdentity(identity)}
        savedRequests={savedRequests}
        saveDialogTab={saveDialogTab}
      />
    </div>
  );
}
