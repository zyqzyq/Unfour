import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button, ConfirmDialog, EmptyState, SplitPane, useI18n } from "@unfour/ui";
import { useApiRequestTabs } from "./hooks/useApiRequestTabs";
import { useApiCollections } from "./hooks/useApiCollections";
import { useApiCollectionFolders } from "./hooks/useApiCollectionFolders";
import {
  getTabSaveState,
  requestTabTitle,
  type ApiRequestTab,
} from "./model/request-tabs";
import type { ApiOpenIntent } from "./model/types";
import { ApiRequestTabs } from "./components/ApiRequestTabs";
import { EnvironmentControl } from "./components/EnvironmentControl";
import { ApiRequestBar } from "./components/ApiRequestBar";
import { ApiRequestEditor } from "./components/ApiRequestEditor";
import { ApiResponseViewer } from "./components/ApiResponseViewer";
import { ApiSaveDialog, type SaveIdentity } from "./components/ApiSaveDialog";
import { ApiCloseRequestDialog } from "./components/ApiCloseRequestDialog";
import { ApiClientSidebar } from "./components/ApiClientSidebar";
import {
  EnvironmentManagerPage,
  type EnvironmentManagerInitialMode,
} from "./components/EnvironmentManagerPage";

type ApiWorkspaceView = "request" | "environments";
type EnvironmentManagerOpenMode =
  | { kind: "manage" }
  | { kind: "new" }
  | { environmentId: string; kind: "edit" };

export function ApiClientPage({
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
    newRequest,
    openHistory,
    openSaved,
    saveTab,
    savedRequests,
    selectTab,
    sendTab,
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
  const [workspaceView, setWorkspaceView] = useState<ApiWorkspaceView>("request");
  const [environmentTabOpen, setEnvironmentTabOpen] = useState(false);
  const [environmentTabDirty, setEnvironmentTabDirty] = useState(false);
  const [environmentCloseDialogOpen, setEnvironmentCloseDialogOpen] = useState(false);
  const [environmentInitialMode, setEnvironmentInitialMode] =
    useState<EnvironmentManagerInitialMode>({ kind: "manage", nonce: 0 });
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState<string | null>(null);
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

  const openEnvironmentManager = useCallback(
    (mode: EnvironmentManagerOpenMode = { kind: "manage" }) => {
      const nextSelectedEnvironmentId =
        mode.kind === "edit"
          ? mode.environmentId
          : mode.kind === "new"
            ? null
            : activeEnvironment?.id ?? selectedEnvironmentId;
      setSelectedEnvironmentId(nextSelectedEnvironmentId ?? null);
      setEnvironmentInitialMode((current) =>
        ({ ...mode, nonce: current.nonce + 1 }) as EnvironmentManagerInitialMode,
      );
      setEnvironmentTabOpen(true);
      setWorkspaceView("environments");
    },
    [activeEnvironment?.id, selectedEnvironmentId],
  );

  const closeEnvironmentTab = useCallback(() => {
    setEnvironmentTabOpen(false);
    setEnvironmentTabDirty(false);
    setEnvironmentCloseDialogOpen(false);
    setWorkspaceView("request");
  }, []);

  const requestCloseEnvironmentTab = useCallback(() => {
    if (environmentTabDirty) {
      setEnvironmentCloseDialogOpen(true);
      return;
    }
    closeEnvironmentTab();
  }, [closeEnvironmentTab, environmentTabDirty]);

  const handleNewRequest = useCallback(() => {
    newRequest();
    setWorkspaceView("request");
  }, [newRequest]);

  const handleSelectTab = useCallback((tabId: string) => {
    selectTab(tabId);
    setWorkspaceView("request");
  }, [selectTab]);

  useEffect(
    () => onActiveSavedRequestChange?.(activeTab?.savedRequestId ?? null),
    [activeTab?.savedRequestId, onActiveSavedRequestChange],
  );

  useEffect(() => {
    if (!openIntent) {
      return;
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- intentional: imperative intent drives view switch
    setWorkspaceView("request");
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
    setWorkspaceView("request");
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
    if (await saveTab(tab)) {
      closeTab(tab.id);
      continuePendingCloseQueue();
    }
  }

  const handleSidebarIntent = useCallback(
    (intent: ApiOpenIntent) => {
      setWorkspaceView("request");
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
        environmentPanelActive={workspaceView === "environments"}
        onEditEnvironment={(environmentId) =>
          openEnvironmentManager({ kind: "edit", environmentId })
        }
        onNewEnvironment={() => openEnvironmentManager({ kind: "new" })}
        onNewRequest={handleNewRequest}
        onOpenEnvironments={() => openEnvironmentManager()}
        onOpenIntent={handleSidebarIntent}
        selectedEnvironmentId={selectedEnvironmentId}
        selectedId={activeTab?.savedRequestId ?? null}
        shellSlot={usesShellSidebar}
        workspaceId={workspaceId}
      />
    ),
    [
      activeTab?.savedRequestId,
      handleNewRequest,
      handleSidebarIntent,
      openEnvironmentManager,
      selectedEnvironmentId,
      usesShellSidebar,
      workspaceView,
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
            activeId={workspaceView === "environments" ? null : state.activeTabId}
            endControl={
              <EnvironmentControl
                activeEnvironmentId={activeEnvironment?.id ?? null}
                onManageEnvironments={() => openEnvironmentManager()}
                onSelectEnvironment={activateEnvironment}
                workspaceId={workspaceId}
              />
            }
            environmentTab={{
              active: workspaceView === "environments",
              dirty: environmentTabDirty,
              onClose: requestCloseEnvironmentTab,
              onSelect: () => setWorkspaceView("environments"),
              open: environmentTabOpen,
            }}
            onClose={requestClose}
            onCloseAll={() => requestCloseMany(state.tabs)}
            onCloseLeft={requestCloseTabsLeftOf}
            onCloseRight={requestCloseTabsRightOf}
            onCloseSaved={requestCloseSavedTabs}
            onNew={handleNewRequest}
            onSelect={handleSelectTab}
            tabs={state.tabs}
          />
          {workspaceView === "environments" && environmentTabOpen ? (
            <EnvironmentManagerPage
              initialMode={environmentInitialMode}
              onDirtyChange={setEnvironmentTabDirty}
              onSelectionChange={setSelectedEnvironmentId}
              workspaceId={workspaceId}
            />
          ) : !activeTab ? (
            <EmptyState className="m-3 flex-1">
              <div className="space-y-2">
                <div>{t("api.empty.noRequestOpen")}</div>
                <Button onClick={handleNewRequest} type="button">
                  {t("common.actions.newRequest")}
                </Button>
              </div>
            </EmptyState>
          ) : (
            <>
              <ApiRequestBar
                onSave={() => requestSave(activeTab)}
                onSend={() => sendTab(activeTab)}
                onUpdate={(patch) => updateDraft(activeTab.id, patch)}
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
          defaultParentFolderId={saveDialogTab.draft.parentFolderId}
          defaultName={saveDialogTab.draft.name}
          folders={folders}
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
      <ConfirmDialog
        confirmLabel={t("api.environment.discard")}
        description={t("api.environment.discardChangesDescription")}
        onConfirm={closeEnvironmentTab}
        onOpenChange={setEnvironmentCloseDialogOpen}
        open={environmentCloseDialogOpen}
        title={t("api.environment.discardChangesTitle")}
      />
    </div>
  );
}
