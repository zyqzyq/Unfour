import Editor from "@monaco-editor/react";
import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  Activity,
  Braces,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock,
  Clipboard,
  Copy,
  Database,
  Download,
  Folder,
  Globe2,
  Home,
  KeyRound,
  Maximize2,
  Minus,
  MoreHorizontal,
  Pencil,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  Plus,
  RefreshCw,
  Save,
  Search,
  Send,
  Square,
  Table2,
  TerminalSquare,
  Trash2,
  Upload,
  X,
  XCircle,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import {
  closeSshSession,
  connectSshSession,
  browseDatabaseTable,
  createCredential,
  createWorkspace,
  deleteApiRequest,
  deleteCredential,
  deleteDatabaseConnection,
  deleteSshConnection,
  executeDatabaseQuery,
  getApiHistoryDetail,
  getDatabaseSchema,
  getSystemHealth,
  getWorkspaceEnvironment,
  getWorkspaceLayout,
  getWorkspaceState,
  deleteWorkspace,
  listDatabaseConnections,
  listApiHistory,
  listSavedApiRequests,
  listSshConnections,
  listSshSessions,
  renameWorkspace,
  exportSshLog,
  inspectCredential,
  resizeSshSession,
  rotateCredential,
  saveApiRequest,
  duplicateApiRequest,
  saveDatabaseConnection,
  saveSshConnection,
  sendApiRequest,
  sendSshInput,
  setActiveWorkspace as setActiveWorkspaceCommand,
  testDatabaseConnection,
  updateWorkspaceEnvironment,
  updateWorkspaceLayout,
} from "./lib/tauri";
import { cn } from "./lib/utils";
import { useWorkspaceStore } from "./store/workspace-store";
import type {
  ApiHistoryItem,
  ApiHistoryDetail,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
  CredentialMetadata,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
  KeyValue,
  SshConnection,
  SshConnectionInput,
  SshSessionEvent,
  SshSessionSummary,
  Workspace,
  WorkspaceTab,
} from "./types";

const methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];

function App() {
  const queryClient = useQueryClient();
  const {
    activeTabId,
    activeWorkspaceId,
    hydrateLayout,
    layoutWorkspaceId,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    setActiveTab,
    setActiveWorkspace,
    setSelectedApiRequest,
    setSelectedDatabaseConnection,
    setSelectedSshConnection,
    sidebarCollapsed,
    snapshotLayout,
    toggleSidebar,
    tabs,
  } = useWorkspaceStore();
  const healthQuery = useQuery({
    queryKey: ["system-health"],
    queryFn: getSystemHealth,
  });
  const workspaceQuery = useQuery({
    queryKey: ["workspaces"],
    queryFn: getWorkspaceState,
  });

  const activeWorkspace =
    workspaceQuery.data?.workspaces.find(
      (workspace) => workspace.id === (activeWorkspaceId || workspaceQuery.data.activeWorkspaceId),
    ) ?? workspaceQuery.data?.workspaces[0];

  const workspaceLayoutQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-layout", activeWorkspace?.id],
    queryFn: () => getWorkspaceLayout(activeWorkspace?.id ?? ""),
  });
  const sidebarSavedRequestsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["api-saved", activeWorkspace?.id],
    queryFn: () => listSavedApiRequests(activeWorkspace?.id ?? ""),
  });
  const sidebarDatabaseConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["database-connections", activeWorkspace?.id],
    queryFn: () => listDatabaseConnections(activeWorkspace?.id ?? ""),
  });
  const sidebarSshConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["ssh-connections", activeWorkspace?.id],
    queryFn: () => listSshConnections(activeWorkspace?.id ?? ""),
  });
  const apiResourceGroups = useMemo(
    () => groupApiRequestsByFolder(sidebarSavedRequestsQuery.data ?? []),
    [sidebarSavedRequestsQuery.data],
  );

  useEffect(() => {
    if (workspaceQuery.data?.activeWorkspaceId && !activeWorkspaceId) {
      setActiveWorkspace(workspaceQuery.data.activeWorkspaceId);
    }
  }, [activeWorkspaceId, setActiveWorkspace, workspaceQuery.data?.activeWorkspaceId]);

  useEffect(() => {
    if (workspaceLayoutQuery.data) {
      hydrateLayout(workspaceLayoutQuery.data);
    }
  }, [hydrateLayout, workspaceLayoutQuery.data]);

  useEffect(() => {
    const items = sidebarSavedRequestsQuery.data;
    if (
      selectedApiRequestId &&
      items &&
      !items.some((item) => item.id === selectedApiRequestId)
    ) {
      setSelectedApiRequest(null);
    }
  }, [selectedApiRequestId, setSelectedApiRequest, sidebarSavedRequestsQuery.data]);

  useEffect(() => {
    const items = sidebarDatabaseConnectionsQuery.data;
    if (
      selectedDatabaseConnectionId &&
      items &&
      !items.some((item) => item.id === selectedDatabaseConnectionId)
    ) {
      setSelectedDatabaseConnection(null);
    }
  }, [
    selectedDatabaseConnectionId,
    setSelectedDatabaseConnection,
    sidebarDatabaseConnectionsQuery.data,
  ]);

  useEffect(() => {
    const items = sidebarSshConnectionsQuery.data;
    if (
      selectedSshConnectionId &&
      items &&
      !items.some((item) => item.id === selectedSshConnectionId)
    ) {
      setSelectedSshConnection(null);
    }
  }, [selectedSshConnectionId, setSelectedSshConnection, sidebarSshConnectionsQuery.data]);

  const layoutMutation = useMutation({
    mutationFn: (workspaceId: string) =>
      updateWorkspaceLayout(workspaceId, snapshotLayout(workspaceId)),
  });

  useEffect(() => {
    if (!activeWorkspace?.id || layoutWorkspaceId !== activeWorkspace.id) {
      return;
    }

    const timeout = window.setTimeout(() => {
      layoutMutation.mutate(activeWorkspace.id);
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [
    activeTabId,
    activeWorkspace?.id,
    layoutWorkspaceId,
    selectedApiRequestId,
    selectedDatabaseConnectionId,
    selectedSshConnectionId,
    sidebarCollapsed,
    tabs,
  ]);

  const createWorkspaceMutation = useMutation({
    mutationFn: createWorkspace,
    onSuccess: (workspace) => {
      setActiveWorkspace(workspace.id);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const activateWorkspaceMutation = useMutation({
    mutationFn: setActiveWorkspaceCommand,
    onSuccess: (state) => {
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const renameWorkspaceMutation = useMutation({
    mutationFn: ({ name, workspaceId }: { name: string; workspaceId: string }) =>
      renameWorkspace(workspaceId, name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
  });

  const deleteWorkspaceMutation = useMutation({
    mutationFn: deleteWorkspace,
    onSuccess: (state) => {
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];

  return (
    <div className="app-shell flex h-screen min-h-[680px] flex-col text-slate-950">
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        createWorkspaceMutation={createWorkspaceMutation}
        deleteWorkspaceMutation={deleteWorkspaceMutation}
        healthReady={healthQuery.data?.storageReady === true}
        onActivateWorkspace={(workspaceId) => activateWorkspaceMutation.mutate(workspaceId)}
        renameWorkspaceMutation={renameWorkspaceMutation}
        syncStrategy={healthQuery.data?.syncStrategy ?? "local-first"}
        workspaces={workspaceQuery.data?.workspaces ?? []}
      />

      <ModuleSwitcher
        activeTabId={activeTabId}
        onSelect={setActiveTab}
        syncStrategy={healthQuery.data?.syncStrategy ?? "local-first"}
        tabs={tabs}
      />

      <div className="flex min-h-0 flex-1">
        <ModuleSidebar
          activeTab={activeTab}
          activeTabId={activeTabId}
          apiResourceGroups={apiResourceGroups}
          collapsed={sidebarCollapsed}
          databaseConnections={sidebarDatabaseConnectionsQuery.data ?? []}
          onSelectApiRequest={(request) => {
            setSelectedApiRequest(request.id);
            setActiveTab("api-main");
          }}
          onSelectDatabaseConnection={(connection) => {
            setSelectedDatabaseConnection(connection.id);
            setActiveTab("database-main");
          }}
          onSelectSshConnection={(connection) => {
            setSelectedSshConnection(connection.id);
            setActiveTab("ssh-main");
          }}
          onToggle={toggleSidebar}
          selectedApiRequestId={selectedApiRequestId}
          selectedDatabaseConnectionId={selectedDatabaseConnectionId}
          selectedSshConnectionId={selectedSshConnectionId}
          sshConnections={sidebarSshConnectionsQuery.data ?? []}
          setActiveTab={setActiveTab}
          setSelectedApiRequest={setSelectedApiRequest}
        />

        <main className="flex min-w-0 flex-1 flex-col bg-slate-100/70">
          <section className="min-h-0 flex-1 overflow-hidden p-3">
          {activeTab.kind === "api" && activeWorkspace && (
            <ApiClientPanel workspaceId={activeWorkspace.id} />
          )}
          {activeTab.kind === "ssh" && activeWorkspace && (
            <SshPanel workspaceId={activeWorkspace.id} />
          )}
          {activeTab.kind === "database" && activeWorkspace && (
            <DatabasePanel workspaceId={activeWorkspace.id} />
          )}
          </section>
        </main>
      </div>
    </div>
  );
}

type PendingMutation<TVariables> = {
  isPending: boolean;
  mutate: (variables: TVariables) => void;
};

function AppTitleBar({
  activeWorkspace,
  createWorkspaceMutation,
  deleteWorkspaceMutation,
  healthReady,
  onActivateWorkspace,
  renameWorkspaceMutation,
  syncStrategy,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  createWorkspaceMutation: PendingMutation<string>;
  deleteWorkspaceMutation: PendingMutation<string>;
  healthReady: boolean;
  onActivateWorkspace: (workspaceId: string) => void;
  renameWorkspaceMutation: PendingMutation<{ name: string; workspaceId: string }>;
  syncStrategy: string;
  workspaces: Workspace[];
}) {
  async function dragWindow(event: React.MouseEvent<HTMLDivElement>) {
    if (event.button !== 0 || !isTauriRuntime()) {
      return;
    }

    await getCurrentWindow().startDragging();
  }

  return (
    <header className="flex h-11 shrink-0 items-center border-b border-slate-200 bg-white text-slate-800">
      <div className="flex h-full items-center gap-1 px-2">
        <TitlebarIconButton ariaLabel="Back" disabled icon={<ChevronLeft size={16} />} />
        <TitlebarIconButton ariaLabel="Forward" disabled icon={<ChevronRight size={16} />} />
        <TitlebarIconButton ariaLabel="Home" icon={<Home size={16} />} />
      </div>

      <div className="h-6 w-px bg-slate-200" />

      <div className="flex h-full min-w-0 items-center px-2">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-teal-600 text-white shadow-sm">
          <Activity size={15} />
        </div>
        <WorkspaceMenu
          activeWorkspace={activeWorkspace}
          createWorkspaceMutation={createWorkspaceMutation}
          deleteWorkspaceMutation={deleteWorkspaceMutation}
          onActivateWorkspace={onActivateWorkspace}
          renameWorkspaceMutation={renameWorkspaceMutation}
          workspaces={workspaces}
        />
      </div>

      <div
        className="flex h-full min-w-0 flex-1 items-center justify-center px-4"
        onMouseDown={dragWindow}
      >
        <div
          className="flex h-8 w-full max-w-[520px] items-center gap-2 rounded-md border border-slate-200 bg-slate-50 px-3 text-sm text-slate-500 shadow-xs"
          onMouseDown={(event) => event.stopPropagation()}
        >
          <Search size={15} />
          <span className="truncate">Search</span>
        </div>
      </div>

      <div className="flex h-full shrink-0 items-center gap-1 px-2">
        <Badge tone={healthReady ? "green" : "amber"}>
          {healthReady ? "local storage" : "checking"}
        </Badge>
        <Badge tone="neutral">{syncStrategy}</Badge>
        <TitlebarIconButton ariaLabel="Notifications" icon={<MoreHorizontal size={16} />} />
        <WindowControls />
      </div>
    </header>
  );
}

function WorkspaceMenu({
  activeWorkspace,
  createWorkspaceMutation,
  deleteWorkspaceMutation,
  onActivateWorkspace,
  renameWorkspaceMutation,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  createWorkspaceMutation: PendingMutation<string>;
  deleteWorkspaceMutation: PendingMutation<string>;
  onActivateWorkspace: (workspaceId: string) => void;
  renameWorkspaceMutation: PendingMutation<{ name: string; workspaceId: string }>;
  workspaces: Workspace[];
}) {
  const [createOpen, setCreateOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [workspaceName, setWorkspaceName] = useState("");
  const [renameDraft, setRenameDraft] = useState(activeWorkspace?.name ?? "");
  const canDelete =
    Boolean(activeWorkspace) && !activeWorkspace?.isDefault && workspaces.length > 1;

  useEffect(() => {
    setRenameDraft(activeWorkspace?.name ?? "");
  }, [activeWorkspace?.id, activeWorkspace?.name]);

  function createWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = workspaceName.trim();
    if (!name) {
      return;
    }
    createWorkspaceMutation.mutate(name);
    setWorkspaceName("");
    setCreateOpen(false);
  }

  function renameWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = renameDraft.trim();
    if (!activeWorkspace || !name || name === activeWorkspace.name) {
      return;
    }
    renameWorkspaceMutation.mutate({ workspaceId: activeWorkspace.id, name });
    setRenameOpen(false);
  }

  function deleteWorkspaceFromDialog() {
    if (!activeWorkspace || !canDelete) {
      return;
    }
    deleteWorkspaceMutation.mutate(activeWorkspace.id);
    setDeleteOpen(false);
  }

  return (
    <>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button
            className="ml-2 max-w-[240px] justify-start gap-1 border-transparent bg-white px-2 font-semibold shadow-none hover:bg-slate-100"
            size="sm"
            type="button"
            variant="outline"
          >
            <span className="min-w-0 truncate">
              {activeWorkspace?.name ?? "No workspace"}
            </span>
            <ChevronDown className="shrink-0 text-slate-500" size={14} />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="start"
            className="z-50 w-72 rounded-md border border-slate-200 bg-white p-1 text-sm text-slate-800 shadow-xl"
            sideOffset={6}
          >
            <DropdownMenu.Label className="px-2 py-1.5 text-xs font-semibold uppercase text-slate-500">
              Workspaces
            </DropdownMenu.Label>
            {workspaces.map((workspace) => (
              <DropdownMenu.Item
                className={cn(
                  "flex min-h-8 cursor-pointer items-center gap-2 rounded px-2 py-1.5 outline-none hover:bg-slate-100 focus:bg-slate-100",
                  activeWorkspace?.id === workspace.id && "bg-teal-50 text-teal-900",
                )}
                key={workspace.id}
                onSelect={() => onActivateWorkspace(workspace.id)}
              >
                <Folder size={14} />
                <span className="min-w-0 flex-1 truncate">{workspace.name}</span>
                {workspace.isDefault && <Badge tone="teal">default</Badge>}
              </DropdownMenu.Item>
            ))}
            {workspaces.length === 0 && (
              <div className="px-2 py-4 text-center text-xs text-slate-500">
                No workspaces
              </div>
            )}
            <DropdownMenu.Separator className="my-1 h-px bg-slate-200" />
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-slate-100 focus:bg-slate-100"
              onSelect={() => setCreateOpen(true)}
            >
              <Plus size={14} />
              New workspace
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 outline-none hover:bg-slate-100 focus:bg-slate-100 disabled:pointer-events-none disabled:opacity-50"
              disabled={!activeWorkspace}
              onSelect={() => setRenameOpen(true)}
            >
              <Pencil size={14} />
              Rename current
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex h-8 cursor-pointer items-center gap-2 rounded px-2 text-rose-700 outline-none hover:bg-rose-50 focus:bg-rose-50 disabled:pointer-events-none disabled:opacity-50"
              disabled={!canDelete}
              onSelect={() => setDeleteOpen(true)}
            >
              <Trash2 size={14} />
              Delete current
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <WorkspaceDialog
        description="Create a workspace for a separate set of API requests, SSH connections, and database resources."
        disabled={createWorkspaceMutation.isPending || !workspaceName.trim()}
        onOpenChange={setCreateOpen}
        onSubmit={createWorkspaceFromDialog}
        open={createOpen}
        setValue={setWorkspaceName}
        submitLabel="Create"
        title="New workspace"
        value={workspaceName}
      />

      <WorkspaceDialog
        description="Rename the active workspace. Existing workspace-scoped records stay attached to it."
        disabled={
          renameWorkspaceMutation.isPending ||
          !renameDraft.trim() ||
          renameDraft.trim() === activeWorkspace?.name
        }
        onOpenChange={setRenameOpen}
        onSubmit={renameWorkspaceFromDialog}
        open={renameOpen}
        setValue={setRenameDraft}
        submitLabel="Rename"
        title="Rename workspace"
        value={renameDraft}
      />

      <Dialog.Root onOpenChange={setDeleteOpen} open={deleteOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-slate-950/30" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-slate-200 bg-white p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-slate-950">
              Delete workspace
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-slate-600">
              Delete {activeWorkspace?.name ?? "this workspace"} locally. The app will switch
              to another available workspace.
            </Dialog.Description>
            <div className="mt-4 flex justify-end gap-2">
              <Dialog.Close asChild>
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              </Dialog.Close>
              <Button
                className="bg-rose-700 hover:bg-rose-800"
                disabled={deleteWorkspaceMutation.isPending || !canDelete}
                onClick={deleteWorkspaceFromDialog}
                type="button"
              >
                <Trash2 size={15} />
                Delete
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </>
  );
}

function WorkspaceDialog({
  description,
  disabled,
  onOpenChange,
  onSubmit,
  open,
  setValue,
  submitLabel,
  title,
  value,
}: {
  description: string;
  disabled: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  open: boolean;
  setValue: (value: string) => void;
  submitLabel: string;
  title: string;
  value: string;
}) {
  return (
    <Dialog.Root onOpenChange={onOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-slate-950/30" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-slate-200 bg-white p-4 shadow-xl">
          <Dialog.Title className="text-base font-semibold text-slate-950">
            {title}
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-slate-600">
            {description}
          </Dialog.Description>
          <form className="mt-4 space-y-4" onSubmit={onSubmit}>
            <Input
              autoFocus
              onChange={(event) => setValue(event.target.value)}
              placeholder="Workspace name"
              value={value}
            />
            <div className="flex justify-end gap-2">
              <Dialog.Close asChild>
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              </Dialog.Close>
              <Button disabled={disabled} type="submit">
                {submitLabel}
              </Button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ModuleSwitcher({
  activeTabId,
  onSelect,
  syncStrategy,
  tabs,
}: {
  activeTabId: string;
  onSelect: (tabId: string) => void;
  syncStrategy: string;
  tabs: WorkspaceTab[];
}) {
  return (
    <nav className="flex h-11 shrink-0 items-center justify-between border-b border-slate-200 bg-slate-50 px-3">
      <div className="flex h-full items-center gap-1">
        {tabs.map((tab) => (
          <button
            className={cn(
              "flex h-8 min-w-[116px] items-center justify-center gap-2 rounded-md px-3 text-sm font-medium transition-colors",
              activeTabId === tab.id
                ? "bg-white text-teal-800 shadow-sm ring-1 ring-inset ring-slate-200"
                : "text-slate-600 hover:bg-white/80 hover:text-slate-950",
            )}
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            type="button"
          >
            {tab.kind === "api" && <Globe2 size={15} />}
            {tab.kind === "ssh" && <TerminalSquare size={15} />}
            {tab.kind === "database" && <Database size={15} />}
            {moduleLabel(tab)}
          </button>
        ))}
      </div>
      <div className="flex items-center gap-2 text-xs text-slate-500">
        <span>No environment</span>
        <ChevronDown size={14} />
        <span className="h-4 w-px bg-slate-300" />
        <span>{syncStrategy}</span>
      </div>
    </nav>
  );
}

function ModuleSidebar({
  activeTab,
  activeTabId,
  apiResourceGroups,
  collapsed,
  databaseConnections,
  onSelectApiRequest,
  onSelectDatabaseConnection,
  onSelectSshConnection,
  onToggle,
  selectedApiRequestId,
  selectedDatabaseConnectionId,
  selectedSshConnectionId,
  setActiveTab,
  setSelectedApiRequest,
  sshConnections,
}: {
  activeTab: WorkspaceTab;
  activeTabId: string;
  apiResourceGroups: ApiResourceGroup[];
  collapsed: boolean;
  databaseConnections: DatabaseConnection[];
  onSelectApiRequest: (request: ApiSavedRequest) => void;
  onSelectDatabaseConnection: (connection: DatabaseConnection | SshConnection) => void;
  onSelectSshConnection: (connection: DatabaseConnection | SshConnection) => void;
  onToggle: () => void;
  selectedApiRequestId: string | null;
  selectedDatabaseConnectionId: string | null;
  selectedSshConnectionId: string | null;
  setActiveTab: (tabId: string) => void;
  setSelectedApiRequest: (requestId: string | null) => void;
  sshConnections: SshConnection[];
}) {
  return (
    <aside
      className={cn(
        "flex min-h-0 shrink-0 flex-col border-r border-slate-200 bg-slate-50 transition-all duration-200",
        collapsed ? "w-[58px]" : "w-[270px]",
      )}
    >
      <div className="flex h-11 shrink-0 items-center gap-2 border-b border-slate-200 px-3">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-white text-slate-600 shadow-xs ring-1 ring-inset ring-slate-200">
          {activeTab.kind === "api" && <Globe2 size={15} />}
          {activeTab.kind === "ssh" && <TerminalSquare size={15} />}
          {activeTab.kind === "database" && <Database size={15} />}
        </div>
        {!collapsed && (
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold text-slate-900">
              {moduleLabel(activeTab)}
            </div>
            <div className="truncate text-xs text-slate-500">{moduleSubtitle(activeTab)}</div>
          </div>
        )}
        <Button
          aria-label="Toggle sidebar"
          className="ml-auto"
          onClick={onToggle}
          size="icon"
          type="button"
          variant="ghost"
        >
          {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {activeTab.kind === "api" && (
          <ApiModuleSidebar
            activeTabId={activeTabId}
            collapsed={collapsed}
            groups={apiResourceGroups}
            onOpenClient={() => {
              setSelectedApiRequest(null);
              setActiveTab("api-main");
            }}
            onSelect={onSelectApiRequest}
            selectedId={selectedApiRequestId}
          />
        )}
        {activeTab.kind === "ssh" && (
          <SshModuleSidebar
            activeTabId={activeTabId}
            collapsed={collapsed}
            items={sshConnections}
            onOpenClient={() => setActiveTab("ssh-main")}
            onSelect={onSelectSshConnection}
            selectedId={selectedSshConnectionId}
          />
        )}
        {activeTab.kind === "database" && (
          <DatabaseModuleSidebar
            activeTabId={activeTabId}
            collapsed={collapsed}
            items={databaseConnections}
            onOpenClient={() => setActiveTab("database-main")}
            onSelect={onSelectDatabaseConnection}
            selectedId={selectedDatabaseConnectionId}
          />
        )}
      </div>
    </aside>
  );
}

function ApiModuleSidebar({
  activeTabId,
  collapsed,
  groups,
  onOpenClient,
  onSelect,
  selectedId,
}: {
  activeTabId: string;
  collapsed: boolean;
  groups: ApiResourceGroup[];
  onOpenClient: () => void;
  onSelect: (request: ApiSavedRequest) => void;
  selectedId: string | null;
}) {
  return (
    <div className="space-y-3">
      <ResourceGroup collapsed={collapsed} icon={<Braces size={16} />} title="Collections">
        <SidebarAction
          collapsed={collapsed}
          icon={<Send size={14} />}
          label="REST Client"
          onClick={onOpenClient}
          selected={activeTabId === "api-main" && (collapsed || !selectedId)}
        />
        <SidebarApiResources
          collapsed={collapsed}
          groups={groups}
          onSelect={onSelect}
          selectedId={selectedId}
        />
        {!collapsed && groups.length === 0 && (
          <SidebarEmptyState>No saved requests</SidebarEmptyState>
        )}
      </ResourceGroup>
      <ResourceGroup collapsed={collapsed} icon={<Globe2 size={16} />} title="Environments">
        {!collapsed && <SidebarEmptyState>No environment selected</SidebarEmptyState>}
      </ResourceGroup>
      <ResourceGroup collapsed={collapsed} icon={<Clock size={16} />} title="History">
        {!collapsed && <SidebarEmptyState>Send a request to build history</SidebarEmptyState>}
      </ResourceGroup>
    </div>
  );
}

function SshModuleSidebar({
  activeTabId,
  collapsed,
  items,
  onOpenClient,
  onSelect,
  selectedId,
}: {
  activeTabId: string;
  collapsed: boolean;
  items: SshConnection[];
  onOpenClient: () => void;
  onSelect: (connection: DatabaseConnection | SshConnection) => void;
  selectedId: string | null;
}) {
  return (
    <div className="space-y-3">
      <ResourceGroup collapsed={collapsed} icon={<TerminalSquare size={16} />} title="SSH">
        <SidebarAction
          collapsed={collapsed}
          icon={<TerminalSquare size={14} />}
          label="SSH Sessions"
          onClick={onOpenClient}
          selected={activeTabId === "ssh-main" && (collapsed || !selectedId)}
        />
        <SidebarConnectionResources
          collapsed={collapsed}
          items={items}
          kind="ssh"
          onSelect={onSelect}
          selectedId={selectedId}
        />
        {!collapsed && items.length === 0 && (
          <SidebarEmptyState>No SSH connections</SidebarEmptyState>
        )}
      </ResourceGroup>
      <ResourceGroup collapsed={collapsed} icon={<Clock size={16} />} title="Sessions">
        {!collapsed && <SidebarEmptyState>No active session selected</SidebarEmptyState>}
      </ResourceGroup>
    </div>
  );
}

function DatabaseModuleSidebar({
  activeTabId,
  collapsed,
  items,
  onOpenClient,
  onSelect,
  selectedId,
}: {
  activeTabId: string;
  collapsed: boolean;
  items: DatabaseConnection[];
  onOpenClient: () => void;
  onSelect: (connection: DatabaseConnection | SshConnection) => void;
  selectedId: string | null;
}) {
  return (
    <div className="space-y-3">
      <ResourceGroup collapsed={collapsed} icon={<Database size={16} />} title="Database">
        <SidebarAction
          collapsed={collapsed}
          icon={<Database size={14} />}
          label="SQL Workspace"
          onClick={onOpenClient}
          selected={activeTabId === "database-main" && (collapsed || !selectedId)}
        />
        <SidebarConnectionResources
          collapsed={collapsed}
          items={items}
          kind="database"
          onSelect={onSelect}
          selectedId={selectedId}
        />
        {!collapsed && items.length === 0 && (
          <SidebarEmptyState>No database connections</SidebarEmptyState>
        )}
      </ResourceGroup>
      <ResourceGroup collapsed={collapsed} icon={<Table2 size={16} />} title="Schema">
        {!collapsed && <SidebarEmptyState>Select a connection</SidebarEmptyState>}
      </ResourceGroup>
    </div>
  );
}

function SidebarEmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-md border border-dashed border-slate-200 bg-white/70 px-2 py-3 text-xs text-slate-500">
      {children}
    </div>
  );
}

function TitlebarIconButton({
  ariaLabel,
  disabled,
  icon,
}: {
  ariaLabel: string;
  disabled?: boolean;
  icon: React.ReactNode;
}) {
  return (
    <Button
      aria-label={ariaLabel}
      className="h-8 w-8 text-slate-600"
      disabled={disabled}
      size="icon"
      type="button"
      variant="ghost"
    >
      {icon}
    </Button>
  );
}

function WindowControls() {
  if (!isTauriRuntime()) {
    return (
      <div className="ml-1 flex items-center gap-1 text-slate-300">
        <Minus size={15} />
        <Square size={13} />
        <X size={15} />
      </div>
    );
  }

  const appWindow = getCurrentWindow();

  return (
    <div className="ml-1 flex items-center">
      <TitlebarWindowButton
        ariaLabel="Minimize"
        icon={<Minus size={16} />}
        onClick={() => void appWindow.minimize()}
      />
      <TitlebarWindowButton
        ariaLabel="Maximize"
        icon={<Maximize2 size={14} />}
        onClick={() => void appWindow.toggleMaximize()}
      />
      <TitlebarWindowButton
        ariaLabel="Close"
        className="hover:bg-rose-600 hover:text-white"
        icon={<X size={16} />}
        onClick={() => void appWindow.close()}
      />
    </div>
  );
}

function TitlebarWindowButton({
  ariaLabel,
  className,
  icon,
  onClick,
}: {
  ariaLabel: string;
  className?: string;
  icon: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={ariaLabel}
      className={cn(
        "flex h-8 w-10 items-center justify-center rounded-sm text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-950",
        className,
      )}
      onClick={onClick}
      type="button"
    >
      {icon}
    </button>
  );
}

function moduleLabel(tab: WorkspaceTab) {
  if (tab.kind === "api") {
    return "API";
  }
  if (tab.kind === "ssh") {
    return "SSH";
  }
  return "Database";
}

function moduleSubtitle(tab: WorkspaceTab) {
  if (tab.kind === "api") {
    return "Collections and requests";
  }
  if (tab.kind === "ssh") {
    return "Connections and sessions";
  }
  return "Connections and schemas";
}

function isTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function ResourceGroup({
  children,
  collapsed,
  icon,
  title,
}: {
  children: React.ReactNode;
  collapsed: boolean;
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <div>
      <div className="mb-1 flex h-7 items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wide text-slate-500">
        {icon}
        {!collapsed && title}
      </div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}

function SidebarAction({
  collapsed,
  icon,
  label,
  onClick,
  selected,
}: {
  collapsed: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  selected: boolean;
}) {
  return (
    <button
      className={cn(
        "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors duration-150",
        selected
          ? "bg-white text-slate-950 shadow-sm ring-1 ring-inset ring-slate-200"
          : "text-slate-600 hover:bg-white hover:text-slate-950",
      )}
      onClick={onClick}
      type="button"
    >
      {icon}
      {!collapsed && <span className="truncate">{label}</span>}
    </button>
  );
}

type ApiResourceGroup = {
  folder: string;
  items: ApiSavedRequest[];
};

function SidebarApiResources({
  collapsed,
  groups,
  onSelect,
  selectedId,
}: {
  collapsed: boolean;
  groups: ApiResourceGroup[];
  onSelect: (request: ApiSavedRequest) => void;
  selectedId: string | null;
}) {
  if (collapsed || groups.length === 0) {
    return null;
  }

  return (
    <div className="mt-1 space-y-2 border-l border-slate-200 pl-2">
      {groups.map((group) => (
        <div key={group.folder}>
          <div className="flex h-6 items-center gap-1.5 px-2 text-[11px] font-medium text-slate-500">
            <Folder size={12} />
            <span className="min-w-0 truncate">{group.folder}</span>
          </div>
          <div className="space-y-1">
            {group.items.map((request) => (
              <SidebarResourceItem
                icon={<Braces size={13} />}
                key={request.id}
                label={request.name}
                meta={request.method}
                onClick={() => onSelect(request)}
                selected={selectedId === request.id}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function SidebarConnectionResources({
  collapsed,
  items,
  kind,
  onSelect,
  selectedId,
}: {
  collapsed: boolean;
  items: Array<DatabaseConnection | SshConnection>;
  kind: "database" | "ssh";
  onSelect: (connection: DatabaseConnection | SshConnection) => void;
  selectedId: string | null;
}) {
  if (collapsed || items.length === 0) {
    return null;
  }

  return (
    <div className="mb-2 mt-1 space-y-1 border-l border-slate-200 pl-2">
      {items.map((connection) => (
        <SidebarResourceItem
          icon={kind === "ssh" ? <TerminalSquare size={13} /> : <Database size={13} />}
          key={connection.id}
          label={connection.name}
          meta={sidebarConnectionMeta(connection, kind)}
          onClick={() => onSelect(connection)}
          selected={selectedId === connection.id}
        />
      ))}
    </div>
  );
}

function SidebarResourceItem({
  icon,
  label,
  meta,
  onClick,
  selected,
}: {
  icon: React.ReactNode;
  label: string;
  meta?: string;
  onClick: () => void;
  selected: boolean;
}) {
  return (
    <button
      className={cn(
        "flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-xs transition-colors duration-150",
        selected
          ? "bg-teal-50 text-teal-900 ring-1 ring-inset ring-teal-200"
          : "text-slate-600 hover:bg-white hover:text-slate-950",
      )}
      onClick={onClick}
      type="button"
    >
      <span className="flex h-4 w-4 shrink-0 items-center justify-center">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {meta && (
        <span className="shrink-0 rounded bg-slate-100 px-1.5 text-[10px] font-medium uppercase leading-5 text-slate-500">
          {meta}
        </span>
      )}
    </button>
  );
}

function groupApiRequestsByFolder(items: ApiSavedRequest[]): ApiResourceGroup[] {
  const folders = new Map<string, ApiSavedRequest[]>();

  for (const item of items) {
    const folder = item.folderPath?.trim() || "Unfiled";
    const folderItems = folders.get(folder) ?? [];
    folderItems.push(item);
    folders.set(folder, folderItems);
  }

  return Array.from(folders.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([folder, folderItems]) => ({
      folder,
      items: [...folderItems].sort((left, right) => left.name.localeCompare(right.name)),
    }));
}

function sidebarConnectionMeta(
  connection: DatabaseConnection | SshConnection,
  kind: "database" | "ssh",
) {
  if (kind === "ssh") {
    const ssh = connection as SshConnection;
    return `${ssh.username}@${ssh.host}`;
  }

  return (connection as DatabaseConnection).driver;
}

function Panel({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("surface-panel flex min-h-0 flex-col rounded-md", className)}>
      {children}
    </section>
  );
}

function PanelHeader({
  actions,
  icon,
  subtitle,
  title,
}: {
  actions?: React.ReactNode;
  icon?: React.ReactNode;
  subtitle?: React.ReactNode;
  title: React.ReactNode;
}) {
  return (
    <div className="surface-header flex min-h-10 items-center justify-between gap-3 px-3 py-2">
      <div className="flex min-w-0 items-center gap-2 text-sm font-semibold">
        {icon}
        <div className="min-w-0">
          <div className="truncate">{title}</div>
          {subtitle && <div className="truncate text-xs font-normal text-slate-500">{subtitle}</div>}
        </div>
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

function ResourceListItem({
  actions,
  children,
  disabled,
  onClick,
  selected,
}: {
  actions?: React.ReactNode;
  children: React.ReactNode;
  disabled?: boolean;
  onClick: () => void;
  selected: boolean;
}) {
  return (
    <div
      className={cn(
        "group flex min-h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors duration-150",
        selected
          ? "bg-teal-50 text-teal-900 ring-1 ring-inset ring-teal-200"
          : "text-slate-700 hover:bg-slate-100 hover:text-slate-950",
      )}
    >
      <button
        className="flex min-w-0 flex-1 items-center gap-2 text-left disabled:cursor-not-allowed"
        disabled={disabled}
        onClick={onClick}
        type="button"
      >
        {children}
      </button>
      {actions && <div className="flex shrink-0 items-center gap-1">{actions}</div>}
    </div>
  );
}

function InlineStatus({
  children,
  className,
  icon,
  tone = "neutral",
}: {
  children: React.ReactNode;
  className?: string;
  icon?: React.ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger";
}) {
  const toneClass = {
    danger: "bg-rose-50 text-rose-800 ring-rose-200",
    neutral: "bg-slate-50 text-slate-700 ring-slate-200",
    success: "bg-emerald-50 text-emerald-800 ring-emerald-200",
    warning: "bg-amber-50 text-amber-800 ring-amber-200",
  }[tone];

  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md px-2 py-2 text-xs ring-1 ring-inset",
        toneClass,
        className,
      )}
    >
      {icon}
      <span className="min-w-0 flex-1">{children}</span>
    </div>
  );
}

function EmptyState({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "empty-state flex items-center justify-center rounded-md px-3 py-4 text-center text-sm",
        className,
      )}
    >
      {children}
    </div>
  );
}

function ApiClientPanel({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const { selectedApiRequestId, setSelectedApiRequest } = useWorkspaceStore();
  const [method, setMethod] = useState("GET");
  const [url, setUrl] = useState("{{base_url}}/get");
  const [name, setName] = useState("Health check");
  const [folderPath, setFolderPath] = useState("Examples");
  const [headers, setHeaders] = useState<KeyValue[]>([
    { key: "Accept", value: "application/json", enabled: true },
  ]);
  const [query, setQuery] = useState<KeyValue[]>([
    { key: "source", value: "{{source}}", enabled: true },
  ]);
  const [body, setBody] = useState("{\n  \"hello\": \"workspace\"\n}");
  const [envVariables, setEnvVariables] = useState<KeyValue[]>([]);
  const [collectionStatus, setCollectionStatus] = useState("");
  const [loadedSavedRequestId, setLoadedSavedRequestId] = useState<string | null>(null);
  const [resultTab, setResultTab] = useState<"response" | "history">("response");
  const [response, setResponse] = useState<ApiResponse | null>(null);

  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });
  const savedQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-saved", workspaceId],
    queryFn: () => listSavedApiRequests(workspaceId),
  });
  const environmentQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["workspace-environment", workspaceId],
    queryFn: () => getWorkspaceEnvironment(workspaceId),
  });

  useEffect(() => {
    if (environmentQuery.data) {
      setEnvVariables(environmentQuery.data.variables);
    }
  }, [environmentQuery.data]);

  useEffect(() => {
    if (!savedQuery.data || !selectedApiRequestId) {
      return;
    }

    const selected = savedQuery.data.find((item) => item.id === selectedApiRequestId);
    if (!selected) {
      setSelectedApiRequest(null);
      setLoadedSavedRequestId(null);
      return;
    }

    if (loadedSavedRequestId !== selected.id) {
      loadSavedRequest(selected);
    }
  }, [loadedSavedRequestId, savedQuery.data, selectedApiRequestId, setSelectedApiRequest]);

  const input = useMemo<ApiRequestInput>(
    () => ({
      workspaceId,
      name,
      folderPath,
      method,
      url,
      headers,
      query,
      body: method === "GET" || method === "HEAD" ? undefined : body,
      bodyKind: "json",
      timeoutMs: 60_000,
    }),
    [body, folderPath, headers, method, name, query, url, workspaceId],
  );

  const sendMutation = useMutation({
    mutationFn: sendApiRequest,
    onSuccess: (result) => {
      setResponse(result);
      setResultTab("response");
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
    },
  });

  const saveMutation = useMutation({
    mutationFn: saveApiRequest,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const duplicateSavedMutation = useMutation({
    mutationFn: (requestId: string) => duplicateApiRequest(workspaceId, requestId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const deleteSavedMutation = useMutation({
    mutationFn: (requestId: string) => deleteApiRequest(workspaceId, requestId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const saveEnvironmentMutation = useMutation({
    mutationFn: (variables: KeyValue[]) => updateWorkspaceEnvironment(workspaceId, variables),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["workspace-environment", workspaceId] }),
  });

  const replayHistoryMutation = useMutation({
    mutationFn: (historyId: string) => getApiHistoryDetail(workspaceId, historyId),
    onSuccess: (history) => {
      setSelectedApiRequest(null);
      setLoadedSavedRequestId(null);
      loadHistoryRequest(history);
    },
  });

  const importCollectionMutation = useMutation({
    mutationFn: async (requests: ApiRequestInput[]) => {
      for (const request of requests) {
        await saveApiRequest({ ...request, workspaceId });
      }
      return requests.length;
    },
    onSuccess: (count) => {
      setCollectionStatus(`Imported ${count} request${count === 1 ? "" : "s"}`);
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
    },
    onError: (error) => setCollectionStatus(formatError(error)),
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    sendMutation.mutate(input);
  }

  function loadRequestDraft(request: ApiRequestInput) {
    setName(request.name ?? `${request.method} ${request.url}`);
    setFolderPath(request.folderPath ?? "");
    setMethod(request.method);
    setUrl(request.url);
    setHeaders(request.headers);
    setQuery(request.query);
    setBody(request.body ?? "");
  }

  function loadSavedRequest(saved: ApiSavedRequest) {
    setSelectedApiRequest(saved.id);
    setLoadedSavedRequestId(saved.id);
    loadRequestDraft(savedRequestToInput(saved, workspaceId));
  }

  function loadHistoryRequest(history: ApiHistoryDetail) {
    loadRequestDraft(historyDetailToInput(history));
  }

  function exportCollection() {
    const payload = {
      version: 1,
      exportedAt: new Date().toISOString(),
      workspaceId,
      savedRequests: (savedQuery.data ?? []).map((item) => ({
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
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json;charset=utf-8",
    });
    const href = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = href;
    link.download = `unfour-api-collection-${new Date()
      .toISOString()
      .slice(0, 10)}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(href);
    setCollectionStatus(`Exported ${payload.savedRequests.length} request${payload.savedRequests.length === 1 ? "" : "s"}`);
  }

  async function importCollection(file: File | undefined) {
    if (!file) {
      return;
    }
    try {
      const parsed = JSON.parse(await file.text());
      const requests = parseCollectionImport(parsed, workspaceId);
      if (!requests.length) {
        setCollectionStatus("No importable requests found");
        return;
      }
      importCollectionMutation.mutate(requests);
    } catch (error) {
      setCollectionStatus(formatError(error));
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[280px_minmax(0,1fr)] gap-3">
      <Panel>
        <PanelHeader
          actions={<Badge tone="neutral">{savedQuery.data?.length ?? 0} saved</Badge>}
          icon={<Globe2 size={16} />}
          title="API Workspace"
        />
        <div className="form-band min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
          <SavedRequestsList
            collectionStatus={collectionStatus}
            importing={importCollectionMutation.isPending}
            items={savedQuery.data ?? []}
            mutatingItemId={
              duplicateSavedMutation.variables ?? deleteSavedMutation.variables ?? null
            }
            onExport={exportCollection}
            onImport={importCollection}
            onLoad={loadSavedRequest}
            onDuplicate={(item) => duplicateSavedMutation.mutate(item.id)}
            onDelete={(item) => {
              if (selectedApiRequestId === item.id) {
                setSelectedApiRequest(null);
                setLoadedSavedRequestId(null);
              }
              deleteSavedMutation.mutate(item.id);
            }}
            selectedId={selectedApiRequestId}
          />
          <div className="subpanel rounded-md p-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
                Environment
              </span>
              <Button
                disabled={saveEnvironmentMutation.isPending}
                onClick={() => saveEnvironmentMutation.mutate(envVariables)}
                size="sm"
                type="button"
                variant="outline"
              >
                <Save size={13} />
                Save
              </Button>
            </div>
            <KeyValueEditor
              items={envVariables}
              maskSensitiveValues
              onChange={setEnvVariables}
              title="Variables"
            />
            <EnvironmentHints variables={envVariables} />
          </div>
        </div>
      </Panel>

      <form className="surface-panel flex min-h-0 flex-col rounded-md" onSubmit={submit}>
        <PanelHeader
          actions={
            <>
              <Button disabled={sendMutation.isPending} type="submit">
                <Send size={15} />
                Send
              </Button>
              <Button
                disabled={saveMutation.isPending}
                onClick={() => saveMutation.mutate(input)}
                type="button"
                variant="outline"
              >
                <Save size={15} />
                Save
              </Button>
            </>
          }
          icon={<Braces size={16} />}
          subtitle={folderPath || "Unfiled"}
          title={name || `${method} request`}
        />

        <div className="form-band space-y-3 border-b border-slate-200 p-3">
          <div className="grid grid-cols-[minmax(0,1fr)_180px] gap-3">
            <FieldGroup title="Request">
              <Input onChange={(event) => setName(event.target.value)} value={name} />
            </FieldGroup>
            <FieldGroup title="Folder">
              <Input
                onChange={(event) => setFolderPath(event.target.value)}
                placeholder="Examples / Auth"
                value={folderPath}
              />
            </FieldGroup>
          </div>
          <div className="grid grid-cols-[112px_minmax(0,1fr)] gap-2">
            <select
              aria-label="HTTP method"
              className="h-9 rounded-md border border-slate-300 bg-white px-2 text-sm font-semibold text-slate-900 shadow-xs outline-none transition-colors hover:border-slate-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-700/15"
              onChange={(event) => setMethod(event.target.value)}
              value={method}
            >
              {methods.map((item) => (
                <option key={item} value={item}>
                  {item}
                </option>
              ))}
            </select>
            <Input onChange={(event) => setUrl(event.target.value)} value={url} />
          </div>
        </div>

        <div className="grid min-h-0 flex-[0.58] grid-cols-[minmax(220px,0.42fr)_minmax(0,1fr)] border-b border-slate-200">
          <div className="min-h-0 space-y-3 overflow-y-auto border-r border-slate-200 p-3">
            <KeyValueEditor items={query} onChange={setQuery} title="Query" />
            <KeyValueEditor items={headers} onChange={setHeaders} title="Headers" />
          </div>

          <div className="flex min-h-0 flex-col">
            <div className="surface-header flex h-10 items-center px-3 text-sm font-medium">
              Body
            </div>
            <div className="min-h-0 flex-1">
              <Editor
                defaultLanguage="json"
                onChange={(value) => setBody(value ?? "")}
                options={{
                  fontSize: 13,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  wordWrap: "on",
                }}
                theme="vs-light"
                value={body}
              />
            </div>
          </div>
        </div>

        <div className="flex min-h-0 flex-[0.42] flex-col">
          <div className="surface-header flex h-10 items-center justify-between px-3">
            <div className="flex items-center gap-1">
              <button
                className={cn(
                  "h-7 rounded-md px-2 text-xs font-semibold transition-colors",
                  resultTab === "response"
                    ? "bg-white text-slate-950 shadow-sm ring-1 ring-inset ring-slate-200"
                    : "text-slate-500 hover:bg-white/70 hover:text-slate-900",
                )}
                onClick={() => setResultTab("response")}
                type="button"
              >
                Response
              </button>
              <button
                className={cn(
                  "h-7 rounded-md px-2 text-xs font-semibold transition-colors",
                  resultTab === "history"
                    ? "bg-white text-slate-950 shadow-sm ring-1 ring-inset ring-slate-200"
                    : "text-slate-500 hover:bg-white/70 hover:text-slate-900",
                )}
                onClick={() => setResultTab("history")}
                type="button"
              >
                History
              </button>
            </div>
            {resultTab === "response" && response && (
              <div className="flex items-center gap-2">
                <Badge tone={response.status < 400 ? "green" : "red"}>
                  {response.status}
                </Badge>
                <Badge tone="neutral">{response.durationMs}ms</Badge>
              </div>
            )}
            {resultTab === "history" && (
              <Badge tone="neutral">{historyQuery.data?.length ?? 0} runs</Badge>
            )}
          </div>

          {resultTab === "response" ? (
            <>
              <ResponseSummary response={response} />
              <ResponseDetails response={response} />
              <div className="min-h-0 flex-1">
                <Editor
                  defaultLanguage="json"
                  options={{
                    fontSize: 12,
                    minimap: { enabled: false },
                    readOnly: true,
                    scrollBeyondLastLine: false,
                    wordWrap: "on",
                  }}
                  theme="vs-light"
                  value={formatResponseBody(response?.body)}
                />
              </div>
            </>
          ) : (
            <HistoryTable
              items={historyQuery.data ?? []}
              loadingReplay={replayHistoryMutation.isPending}
              onReplay={(item) => {
                setResultTab("response");
                replayHistoryMutation.mutate(item.id);
              }}
            />
          )}
        </div>
      </form>
    </div>
  );
}

function SavedRequestsList({
  collectionStatus,
  importing,
  items,
  mutatingItemId,
  onDelete,
  onDuplicate,
  onExport,
  onImport,
  onLoad,
  selectedId,
}: {
  collectionStatus: string;
  importing: boolean;
  items: ApiSavedRequest[];
  mutatingItemId: string | null;
  onDelete: (item: ApiSavedRequest) => void;
  onDuplicate: (item: ApiSavedRequest) => void;
  onExport: () => void;
  onImport: (file: File | undefined) => void;
  onLoad: (item: ApiSavedRequest) => void;
  selectedId: string | null;
}) {
  const groups = groupSavedRequests(items);

  return (
    <div className="subpanel rounded-md p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
          Saved
        </span>
        <Badge tone="neutral">{items.length}</Badge>
      </div>
      <div className="mb-2 grid grid-cols-2 gap-2">
        <Button
          disabled={items.length === 0}
          onClick={onExport}
          size="sm"
          type="button"
          variant="outline"
        >
          <Download size={13} />
          Export
        </Button>
        <label>
          <input
            accept="application/json"
            className="sr-only"
            disabled={importing}
            onChange={(event) => {
              onImport(event.target.files?.[0]);
              event.target.value = "";
            }}
            type="file"
          />
          <span
            className={cn(
              "inline-flex h-8 w-full cursor-pointer items-center justify-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-xs font-medium text-slate-800 transition-colors hover:border-slate-400 hover:bg-slate-50",
              importing && "pointer-events-none opacity-50",
            )}
          >
            <Upload size={13} />
            Import
          </span>
        </label>
      </div>
      {collectionStatus && (
        <InlineStatus className="mb-2 truncate py-1" tone="neutral">
          {collectionStatus}
        </InlineStatus>
      )}
      <div className="max-h-64 space-y-3 overflow-y-auto">
        {groups.map((group) => (
          <div key={group.folder}>
            <div className="mb-1 flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
              <Folder size={12} />
              <span className="min-w-0 flex-1 truncate">{group.folder}</span>
              <Badge tone="neutral">{group.items.length}</Badge>
            </div>
            <div className="space-y-1">
              {group.items.map((item) => (
                <ResourceListItem
                  actions={
                    <>
                      <Button
                        aria-label={`Duplicate ${item.name}`}
                        disabled={mutatingItemId === item.id}
                        onClick={() => onDuplicate(item)}
                        size="icon"
                        type="button"
                        variant="ghost"
                      >
                        <Copy size={12} />
                      </Button>
                      <Button
                        aria-label={`Delete ${item.name}`}
                        disabled={mutatingItemId === item.id}
                        onClick={() => onDelete(item)}
                        size="icon"
                        type="button"
                        variant="ghost"
                      >
                        <Trash2 size={12} />
                      </Button>
                    </>
                  }
                  key={item.id}
                  onClick={() => onLoad(item)}
                  selected={selectedId === item.id}
                >
                  <Badge tone="teal">{item.method}</Badge>
                  <span className="min-w-0 flex-1 truncate text-xs">{item.name}</span>
                </ResourceListItem>
              ))}
            </div>
          </div>
        ))}
        {items.length === 0 && (
          <EmptyState className="py-3 text-xs">No saved requests</EmptyState>
        )}
      </div>
    </div>
  );
}

function FieldGroup({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <label className="block space-y-2">
      <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">{title}</span>
      {children}
    </label>
  );
}

function CredentialReferenceControl({
  kind,
  label,
  onChange,
  value,
  workspaceId,
}: {
  kind: string;
  label: string;
  onChange: (credentialRef: string | null) => void;
  value?: string | null;
  workspaceId: string;
}) {
  const [credentialLabel, setCredentialLabel] = useState(label);
  const [secret, setSecret] = useState("");
  const [metadata, setMetadata] = useState<CredentialMetadata | null>(null);
  const [status, setStatus] = useState("");
  const credentialRef = value?.trim() ?? "";

  useEffect(() => {
    setMetadata(null);
    setStatus("");
  }, [credentialRef]);

  const createMutation = useMutation({
    mutationFn: () =>
      createCredential({
        workspaceId,
        kind,
        label: credentialLabel.trim() || label,
        secret,
      }),
    onSuccess: (created) => {
      setMetadata(created);
      setSecret("");
      setStatus("Credential reference created");
      onChange(created.credentialRef);
    },
  });
  const inspectMutation = useMutation({
    mutationFn: () => inspectCredential({ workspaceId, credentialRef }),
    onSuccess: (inspected) => {
      setMetadata(inspected);
      setStatus("Credential reference verified");
    },
  });
  const rotateMutation = useMutation({
    mutationFn: () => rotateCredential({ workspaceId, credentialRef, secret }),
    onSuccess: (rotated) => {
      setMetadata(rotated);
      setSecret("");
      setStatus("Credential rotated");
    },
  });
  const deleteMutation = useMutation({
    mutationFn: () => deleteCredential({ workspaceId, credentialRef }),
    onSuccess: () => {
      setMetadata(null);
      setSecret("");
      setStatus("Credential deleted");
      onChange(null);
    },
  });
  const error =
    createMutation.error ??
    inspectMutation.error ??
    rotateMutation.error ??
    deleteMutation.error;
  const isPending =
    createMutation.isPending ||
    inspectMutation.isPending ||
    rotateMutation.isPending ||
    deleteMutation.isPending;

  return (
    <div className="space-y-2 rounded-md border border-slate-200 bg-slate-50 p-2">
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
        <KeyRound size={13} />
        Credential
      </div>
      <Input
        onChange={(event) => onChange(event.target.value.trim() || null)}
        placeholder="Create or paste a credential reference"
        value={credentialRef}
      />
      <div className="grid grid-cols-2 gap-2">
        <Input
          onChange={(event) => setCredentialLabel(event.target.value)}
          placeholder={label}
          value={credentialLabel}
        />
        <Input
          onChange={(event) => setSecret(event.target.value)}
          placeholder="Secret value"
          type="password"
          value={secret}
        />
      </div>
      <div className="grid grid-cols-4 gap-1.5">
        <Button
          aria-label="Create credential"
          disabled={!secret || isPending}
          onClick={() => createMutation.mutate()}
          size="icon"
          title="Create credential"
          type="button"
          variant="outline"
        >
          <Plus size={13} />
        </Button>
        <Button
          aria-label="Check credential"
          disabled={!credentialRef || isPending}
          onClick={() => inspectMutation.mutate()}
          size="icon"
          title="Check credential"
          type="button"
          variant="outline"
        >
          <CheckCircle2 size={13} />
        </Button>
        <Button
          aria-label="Rotate credential"
          disabled={!credentialRef || !secret || isPending}
          onClick={() => rotateMutation.mutate()}
          size="icon"
          title="Rotate credential"
          type="button"
          variant="outline"
        >
          <RefreshCw size={13} />
        </Button>
        <Button
          aria-label="Delete credential"
          disabled={!credentialRef || isPending}
          onClick={() => deleteMutation.mutate()}
          size="icon"
          title="Delete credential"
          type="button"
          variant="ghost"
        >
          <Trash2 size={13} />
        </Button>
      </div>
      {metadata && (
        <InlineStatus className="py-1" tone="success">
          {metadata.kind}
        </InlineStatus>
      )}
      {(status || error) && (
        <InlineStatus className="py-1" tone={error ? "danger" : "neutral"}>
          {error ? formatError(error) : status}
        </InlineStatus>
      )}
    </div>
  );
}

function KeyValueEditor({
  items,
  maskSensitiveValues = false,
  onChange,
  title,
}: {
  items: KeyValue[];
  maskSensitiveValues?: boolean;
  onChange: (items: KeyValue[]) => void;
  title: string;
}) {
  function update(index: number, patch: Partial<KeyValue>) {
    onChange(items.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)));
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">{title}</span>
        <Button
          onClick={() => onChange([...items, { key: "", value: "", enabled: true }])}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus size={13} />
          Add
        </Button>
      </div>
      <div className="space-y-2">
        {items.map((item, index) => (
          <div className="grid grid-cols-[20px_1fr_1fr] gap-2" key={`${title}-${index}`}>
            <input
              checked={item.enabled}
              className="mt-2 h-4 w-4"
              onChange={(event) => update(index, { enabled: event.target.checked })}
              type="checkbox"
            />
            <Input
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder="Key"
              value={item.key}
            />
            <Input
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder="Value"
              type={maskSensitiveValues && isSensitiveKey(item.key) ? "password" : "text"}
              value={item.value}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

function EnvironmentHints({ variables }: { variables: KeyValue[] }) {
  const duplicateKeys = duplicateEnvironmentKeys(variables);
  const sensitiveKeys = variables
    .filter((item) => item.enabled && isSensitiveKey(item.key) && item.value.trim())
    .map((item) => item.key.trim());

  if (!duplicateKeys.length && !sensitiveKeys.length) {
    return null;
  }

  return (
    <div className="mt-2 space-y-1 text-xs">
      {duplicateKeys.length > 0 && (
        <div className="rounded-md bg-amber-50 px-2 py-1 text-amber-800 ring-1 ring-inset ring-amber-200">
          Duplicate variables: {duplicateKeys.join(", ")}
        </div>
      )}
      {sensitiveKeys.length > 0 && (
        <div className="rounded-md bg-slate-50 px-2 py-1 text-slate-600 ring-1 ring-inset ring-slate-200">
          Sensitive-looking values are masked locally: {sensitiveKeys.join(", ")}
        </div>
      )}
    </div>
  );
}

function parseKeyValues(value: string): KeyValue[] {
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) {
      return parsed.filter(
        (item): item is KeyValue =>
          typeof item?.key === "string" &&
          typeof item?.value === "string" &&
          typeof item?.enabled === "boolean",
      );
    }
  } catch {
    return [];
  }

  return [];
}

function savedRequestToInput(saved: ApiSavedRequest, workspaceId: string): ApiRequestInput {
  return {
    workspaceId,
    name: saved.name,
    folderPath: saved.folderPath,
    method: saved.method,
    url: saved.url,
    headers: parseKeyValues(saved.headersJson),
    query: parseKeyValues(saved.queryJson),
    body: saved.body ?? undefined,
    bodyKind: saved.bodyKind,
    timeoutMs: 60_000,
  };
}

function historyDetailToInput(history: ApiHistoryDetail): ApiRequestInput {
  return {
    workspaceId: history.workspaceId,
    name: history.name ?? `${history.method} ${history.url}`,
    folderPath: null,
    method: history.method,
    url: history.url,
    headers: parseKeyValues(history.requestHeadersJson),
    query: parseKeyValues(history.requestQueryJson),
    body: history.requestBody ?? undefined,
    bodyKind: "json",
    timeoutMs: 60_000,
  };
}

function parseCollectionImport(value: unknown, workspaceId: string): ApiRequestInput[] {
  const rawItems = Array.isArray(value)
    ? value
    : typeof value === "object" && value !== null && "savedRequests" in value
      ? (value as { savedRequests?: unknown }).savedRequests
      : [];
  if (!Array.isArray(rawItems)) {
    return [];
  }

  return rawItems
    .map((item) => normalizeImportedRequest(item, workspaceId))
    .filter((item): item is ApiRequestInput => item !== null);
}

function normalizeImportedRequest(item: unknown, workspaceId: string): ApiRequestInput | null {
  if (typeof item !== "object" || item === null) {
    return null;
  }
  const candidate = item as Partial<ApiRequestInput>;
  if (typeof candidate.method !== "string" || typeof candidate.url !== "string") {
    return null;
  }

  return {
    workspaceId,
    name: typeof candidate.name === "string" ? candidate.name : undefined,
    folderPath: typeof candidate.folderPath === "string" ? candidate.folderPath : null,
    method: candidate.method.toUpperCase(),
    url: candidate.url,
    headers: Array.isArray(candidate.headers) ? sanitizeKeyValues(candidate.headers) : [],
    query: Array.isArray(candidate.query) ? sanitizeKeyValues(candidate.query) : [],
    body: typeof candidate.body === "string" ? candidate.body : undefined,
    bodyKind: typeof candidate.bodyKind === "string" ? candidate.bodyKind : "json",
    timeoutMs: typeof candidate.timeoutMs === "number" ? candidate.timeoutMs : 60_000,
  };
}

function sanitizeKeyValues(items: unknown[]): KeyValue[] {
  return items
    .filter(isKeyValueLike)
    .map((item) => ({
      key: item.key ?? "",
      value: item.value ?? "",
      enabled: typeof item.enabled === "boolean" ? item.enabled : true,
    }));
}

function isKeyValueLike(item: unknown): item is Partial<KeyValue> {
  if (typeof item !== "object" || item === null) {
    return false;
  }
  const candidate = item as Record<string, unknown>;
  return typeof candidate.key === "string" && typeof candidate.value === "string";
}

function groupSavedRequests(items: ApiSavedRequest[]) {
  const groups = new Map<string, ApiSavedRequest[]>();
  for (const item of items) {
    const folder = item.folderPath?.trim() || "Unfiled";
    groups.set(folder, [...(groups.get(folder) ?? []), item]);
  }

  return Array.from(groups.entries())
    .sort(([left], [right]) => {
      if (left === "Unfiled") return -1;
      if (right === "Unfiled") return 1;
      return left.localeCompare(right);
    })
    .map(([folder, groupItems]) => ({
      folder,
      items: groupItems.sort((left, right) => left.name.localeCompare(right.name)),
    }));
}

function duplicateEnvironmentKeys(variables: KeyValue[]) {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const variable of variables) {
    const key = variable.key.trim().toLowerCase();
    if (!key || !variable.enabled) {
      continue;
    }
    if (seen.has(key)) {
      duplicates.add(variable.key.trim());
    }
    seen.add(key);
  }
  return Array.from(duplicates);
}

function isSensitiveKey(key: string) {
  return /(token|secret|password|passwd|api[_-]?key|auth|credential)/i.test(key);
}

function formatByteSize(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const kb = bytes / 1024;
  if (kb < 1024) {
    return `${kb.toFixed(kb >= 10 ? 0 : 1)} KB`;
  }
  const mb = kb / 1024;
  return `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`;
}

const columnHelper = createColumnHelper<ApiHistoryItem>();

function ResponseSummary({ response }: { response: ApiResponse | null }) {
  if (!response) {
    return (
      <div className="grid grid-cols-3 border-b border-slate-200 bg-slate-50 text-xs text-slate-500">
        <div className="px-3 py-2">Headers -</div>
        <div className="px-3 py-2">Size -</div>
        <div className="px-3 py-2">Timing -</div>
      </div>
    );
  }

  const bodySize = formatByteSize(new TextEncoder().encode(response.body).length);
  const headerSize = formatByteSize(
    new TextEncoder().encode(
      response.headers.map((item) => `${item.key}: ${item.value}`).join("\r\n"),
    ).length,
  );
  const cookies = response.headers.filter((item) => item.key.toLowerCase() === "set-cookie");

  return (
    <div className="grid grid-cols-3 border-b border-slate-200 bg-slate-50 text-xs text-slate-600">
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-slate-800">{response.headers.length}</span> headers
        {cookies.length > 0 && <span>, {cookies.length} cookies</span>}
      </div>
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-slate-800">{bodySize}</span> body, {headerSize} headers
      </div>
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-slate-800">{response.durationMs}ms</span> total
      </div>
    </div>
  );
}

function ResponseDetails({ response }: { response: ApiResponse | null }) {
  if (!response) {
    return null;
  }

  const cookies = response.headers.filter((item) => item.key.toLowerCase() === "set-cookie");
  const bodyBytes = new TextEncoder().encode(response.body).length;
  const headerBytes = new TextEncoder().encode(
    response.headers.map((item) => `${item.key}: ${item.value}`).join("\r\n"),
  ).length;

  return (
    <div className="grid max-h-32 grid-cols-[1.4fr_1fr] overflow-hidden border-b border-slate-200 text-xs">
      <div className="min-w-0 overflow-auto border-r border-slate-200 p-2">
        <div className="mb-1 font-semibold uppercase tracking-wide text-slate-500">Headers</div>
        <div className="space-y-1">
          {response.headers.map((header) => (
            <div className="grid grid-cols-[120px_minmax(0,1fr)] gap-2" key={`${header.key}-${header.value}`}>
              <span className="truncate font-medium text-slate-700">{header.key}</span>
              <span className="truncate text-slate-500">{header.value}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="min-w-0 overflow-auto p-2">
        <div className="mb-1 font-semibold uppercase tracking-wide text-slate-500">Timing / Size</div>
        <div className="grid grid-cols-2 gap-2 text-slate-600">
          <Metric label="Total" value={`${response.durationMs}ms`} />
          <Metric label="Status" value={`${response.status} ${response.statusText}`} />
          <Metric label="Body" value={formatByteSize(bodyBytes)} />
          <Metric label="Headers" value={formatByteSize(headerBytes)} />
          <Metric label="Cookies" value={String(cookies.length)} />
        </div>
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md bg-slate-50 px-2 py-1 ring-1 ring-inset ring-slate-200">
      <div className="text-[10px] uppercase text-slate-400">{label}</div>
      <div className="truncate font-medium text-slate-700">{value}</div>
    </div>
  );
}

function HistoryTable({
  items,
  loadingReplay,
  onReplay,
}: {
  items: ApiHistoryItem[];
  loadingReplay: boolean;
  onReplay: (item: ApiHistoryItem) => void;
}) {
  const columns = useMemo(
    () => [
      columnHelper.display({
        cell: (info) => (
          <Button
            disabled={loadingReplay}
            onClick={() => onReplay(info.row.original)}
            size="sm"
            type="button"
            variant="ghost"
          >
            Load
          </Button>
        ),
        header: "",
        id: "replay",
      }),
      columnHelper.accessor("method", {
        cell: (info) => <Badge tone="teal">{info.getValue()}</Badge>,
        header: "Method",
      }),
      columnHelper.accessor("status", {
        cell: (info) => {
          const status = info.getValue();
          return status ? <Badge tone={status < 400 ? "green" : "red"}>{status}</Badge> : "-";
        },
        header: "Status",
      }),
      columnHelper.accessor("url", {
        cell: (info) => <span className="block max-w-[190px] truncate">{info.getValue()}</span>,
        header: "URL",
      }),
      columnHelper.accessor("durationMs", {
        cell: (info) => {
          const value = info.getValue();
          return value ? `${value}ms` : "-";
        },
        header: "Time",
      }),
    ],
    [loadingReplay, onReplay],
  );
  const table = useReactTable({
    columns,
    data: items,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <table className="data-table w-full text-left text-xs">
        <thead className="sticky top-0">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <th className="border-b border-slate-200 px-3 py-2 font-medium" key={header.id}>
                  {flexRender(header.column.columnDef.header, header.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr className="border-b" key={row.id}>
              {row.getVisibleCells().map((cell) => (
                <td className="px-3 py-2" key={cell.id}>
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {items.length === 0 && (
        <EmptyState className="m-3 h-24">No requests yet</EmptyState>
      )}
    </div>
  );
}

function SshPanel({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const { selectedSshConnectionId: selectedConnectionId, setSelectedSshConnection } =
    useWorkspaceStore();
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [terminalInput, setTerminalInput] = useState("whoami\n");
  const [terminalEvents, setTerminalEvents] = useState<SshSessionEvent[]>([]);
  const [exportedLog, setExportedLog] = useState<string | null>(null);
  const [form, setForm] = useState<SshConnectionInput>({
    workspaceId,
    name: "Deploy host",
    host: "example.internal",
    port: 22,
    username: "deploy",
    authKind: "password",
    credentialRef: null,
  });

  const connectionsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["ssh-connections", workspaceId],
    queryFn: () => listSshConnections(workspaceId),
  });
  const sessionsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["ssh-sessions", workspaceId],
    queryFn: () => listSshSessions(workspaceId),
  });

  const selectedConnection: SshConnection | null =
    connectionsQuery.data?.find((item) => item.id === selectedConnectionId) ?? null;
  const activeSession: SshSessionSummary | null =
    sessionsQuery.data?.find((item) => item.sessionId === activeSessionId) ?? null;

  useEffect(() => {
    setForm((current) => ({ ...current, workspaceId }));
  }, [workspaceId]);

  useEffect(() => {
    if (!connectionsQuery.data?.length) {
      setSelectedSshConnection(null);
      return;
    }
    if (
      !selectedConnectionId ||
      !connectionsQuery.data.some((connection) => connection.id === selectedConnectionId)
    ) {
      setSelectedSshConnection(connectionsQuery.data[0].id);
    }
  }, [connectionsQuery.data, selectedConnectionId, setSelectedSshConnection]);

  useEffect(() => {
    if (!selectedConnection) {
      return;
    }
    setForm({
      id: selectedConnection.id,
      workspaceId,
      name: selectedConnection.name,
      host: selectedConnection.host,
      port: selectedConnection.port,
      username: selectedConnection.username,
      authKind: selectedConnection.authKind,
      keyPath: selectedConnection.keyPath,
      credentialRef: selectedConnection.credentialRef,
    });
  }, [selectedConnection, workspaceId]);

  const saveMutation = useMutation({
    mutationFn: saveSshConnection,
    onSuccess: (connection) => {
      setSelectedSshConnection(connection.id);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteSshConnection(workspaceId, connectionId),
    onSuccess: () => {
      setSelectedSshConnection(null);
      setActiveSessionId(null);
      setTerminalEvents([]);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const connectMutation = useMutation({
    mutationFn: (connectionId: string) =>
      connectSshSession({ workspaceId, connectionId, cols: 120, rows: 32 }),
    onSuccess: (session) => {
      setActiveSessionId(session.sessionId);
      setTerminalEvents([
        {
          sessionId: session.sessionId,
          kind: "output",
          data: `Connected to ${session.username}@${session.host}. PTY ${session.cols}x${session.rows} allocated.\r\n`,
          createdAt: session.createdAt,
        },
      ]);
      setExportedLog(null);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const inputMutation = useMutation({
    mutationFn: () =>
      sendSshInput({
        workspaceId,
        sessionId: activeSessionId ?? "",
        data: terminalInput,
      }),
    onSuccess: (event) => {
      setTerminalEvents((current) => [
        ...current,
        {
          sessionId: event.sessionId,
          kind: "input",
          data: terminalInput,
          createdAt: new Date().toISOString(),
        },
        event,
      ]);
      setTerminalInput("");
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const resizeMutation = useMutation({
    mutationFn: () =>
      resizeSshSession({
        workspaceId,
        sessionId: activeSessionId ?? "",
        cols: activeSession?.cols === 120 ? 140 : 120,
        rows: activeSession?.rows === 32 ? 40 : 32,
      }),
    onSuccess: (event) => {
      setTerminalEvents((current) => [...current, event]);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const closeMutation = useMutation({
    mutationFn: () => closeSshSession({ workspaceId, sessionId: activeSessionId ?? "" }),
    onSuccess: (session) => {
      setTerminalEvents((current) => [
        ...current,
        {
          sessionId: session.sessionId,
          kind: "close",
          data: "SSH session closed.\r\n",
          createdAt: session.updatedAt,
        },
      ]);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const exportMutation = useMutation({
    mutationFn: () => exportSshLog({ workspaceId, sessionId: activeSessionId ?? "" }),
    onSuccess: (log) => setExportedLog(log.content),
  });

  function updateForm(patch: Partial<SshConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function newConnection() {
    setSelectedSshConnection(null);
    setForm({
      workspaceId,
      name: "Deploy host",
      host: "example.internal",
      port: 22,
      username: "deploy",
      authKind: "password",
      credentialRef: null,
    });
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate({
      ...form,
      credentialRef: form.credentialRef?.trim() || null,
      keyPath: form.keyPath?.trim() || null,
    });
  }

  function connectSelectedConnection() {
    if (selectedConnectionId) {
      connectMutation.mutate(selectedConnectionId);
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[280px_minmax(0,1fr)] gap-3">
      <Panel>
        <PanelHeader
          actions={
            <>
              <Badge tone="green">session mvp</Badge>
              <Button aria-label="New SSH connection" onClick={newConnection} size="icon" type="button" variant="ghost">
                <Plus size={15} />
              </Button>
            </>
          }
          icon={<TerminalSquare size={16} />}
          title="SSH Connections"
        />

        <form className="form-band space-y-3 border-b border-slate-200 p-3" onSubmit={submitConnection}>
          <FieldGroup title="Name">
            <Input onChange={(event) => updateForm({ name: event.target.value })} value={form.name} />
          </FieldGroup>
          <div className="grid grid-cols-[1fr_84px] gap-2">
            <FieldGroup title="Host">
              <Input onChange={(event) => updateForm({ host: event.target.value })} value={form.host} />
            </FieldGroup>
            <FieldGroup title="Port">
              <Input
                onChange={(event) =>
                  updateForm({ port: event.target.value ? Number(event.target.value) : null })
                }
                type="number"
                value={form.port ?? ""}
              />
            </FieldGroup>
          </div>
          <FieldGroup title="Username">
            <Input onChange={(event) => updateForm({ username: event.target.value })} value={form.username} />
          </FieldGroup>
          <FieldGroup title="Auth">
            <select
              className="h-9 w-full rounded-md border border-slate-300 bg-white px-2 text-sm text-slate-950 shadow-xs outline-none transition-colors hover:border-slate-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-700/15"
              onChange={(event) =>
                updateForm({
                  authKind: event.target.value as SshConnectionInput["authKind"],
                  keyPath: event.target.value === "private-key" ? form.keyPath : null,
                })
              }
              value={form.authKind}
            >
              <option value="password">Password</option>
              <option value="private-key">Private key</option>
            </select>
          </FieldGroup>
          {form.authKind === "private-key" && (
            <FieldGroup title="Key Path">
              <Input
                onChange={(event) => updateForm({ keyPath: event.target.value })}
                placeholder="C:\\Users\\me\\.ssh\\id_ed25519"
                value={form.keyPath ?? ""}
              />
            </FieldGroup>
          )}
          <CredentialReferenceControl
            kind={form.authKind === "private-key" ? "ssh-key-passphrase" : "ssh-password"}
            label={`${form.name || "SSH"} credential`}
            onChange={(credentialRef) => updateForm({ credentialRef })}
            value={form.credentialRef}
            workspaceId={workspaceId}
          />

          <div className="flex items-center gap-2">
            <Button disabled={saveMutation.isPending} type="submit">
              <Save size={15} />
              Save
            </Button>
            <Button
              aria-label="Delete SSH connection"
              disabled={!selectedConnectionId || deleteMutation.isPending}
              onClick={() => selectedConnectionId && deleteMutation.mutate(selectedConnectionId)}
              size="icon"
              type="button"
              variant="ghost"
            >
              <Trash2 size={15} />
            </Button>
          </div>
          {(saveMutation.error || deleteMutation.error) && (
            <InlineStatus icon={<XCircle size={14} />} tone="danger">
              {formatError(saveMutation.error ?? deleteMutation.error)}
            </InlineStatus>
          )}
        </form>

        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
              Saved Connections
            </span>
            <Badge tone="neutral">{connectionsQuery.data?.length ?? 0}</Badge>
          </div>
          <div className="space-y-1">
            {connectionsQuery.data?.map((connection) => (
              <ResourceListItem
                key={connection.id}
                onClick={() => setSelectedSshConnection(connection.id)}
                selected={selectedConnectionId === connection.id}
              >
                <span className="min-w-0 flex-1 truncate">{connection.name}</span>
                <Badge tone={connection.authKind === "password" ? "amber" : "teal"}>
                  {connection.authKind}
                </Badge>
              </ResourceListItem>
            ))}
            {connectionsQuery.data?.length === 0 && (
              <EmptyState>No SSH connections</EmptyState>
            )}
          </div>
        </div>
      </Panel>
      <Panel>
        <PanelHeader
          actions={
            <>
              {activeSession && (
                <Badge tone={activeSession.status === "active" ? "green" : "neutral"}>
                  {activeSession.status}
                </Badge>
              )}
              <Button
                disabled={!selectedConnectionId || connectMutation.isPending}
                onClick={connectSelectedConnection}
                size="sm"
                type="button"
              >
                <Play size={14} />
                Connect
              </Button>
              <Button
                aria-label="Resize SSH session"
                disabled={!activeSessionId || activeSession?.status !== "active" || resizeMutation.isPending}
                onClick={() => resizeMutation.mutate()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <RefreshCw size={15} />
              </Button>
              <Button
                aria-label="Export SSH log"
                disabled={!activeSessionId || exportMutation.isPending}
                onClick={() => exportMutation.mutate()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Download size={15} />
              </Button>
              <Button
                aria-label="Close SSH session"
                disabled={!activeSessionId || activeSession?.status !== "active" || closeMutation.isPending}
                onClick={() => closeMutation.mutate()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <XCircle size={15} />
              </Button>
            </>
          }
          icon={<TerminalSquare size={16} />}
          subtitle={
            activeSession
              ? `${activeSession.username}@${activeSession.host} ${activeSession.cols}x${activeSession.rows}`
              : selectedConnection
                ? `${selectedConnection.username}@${selectedConnection.host}`
                : undefined
          }
          title="SSH Session"
        />
        <div className="flex min-h-0 flex-1 flex-col bg-slate-950">
          <div className="min-h-0 flex-1 overflow-auto p-4 font-mono text-xs leading-6 text-emerald-100">
            {terminalEvents.length === 0 ? (
              <div className="text-slate-500">Select a connection and start a session.</div>
            ) : (
              terminalEvents.map((event, index) => (
                <div
                  className={cn(
                    "whitespace-pre-wrap break-words",
                    event.kind === "input" && "text-sky-200",
                    event.kind === "resize" && "text-amber-200",
                    event.kind === "close" && "text-slate-300",
                  )}
                  key={`${event.sessionId}-${event.kind}-${index}`}
                >
                  {event.kind === "input" ? `$ ${event.data}` : event.data}
                </div>
              ))
            )}
          </div>
          <div className="border-t border-slate-800 p-3">
            <div className="flex gap-2">
              <Input
                className="border-slate-700 bg-slate-900 font-mono text-emerald-100 placeholder:text-slate-500"
                disabled={!activeSessionId || activeSession?.status !== "active"}
                onChange={(event) => setTerminalInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
                    inputMutation.mutate();
                  }
                }}
                placeholder="Command input"
                value={terminalInput}
              />
              <Button
                disabled={
                  !activeSessionId ||
                  activeSession?.status !== "active" ||
                  !terminalInput ||
                  inputMutation.isPending
                }
                onClick={() => inputMutation.mutate()}
                type="button"
              >
                <Send size={14} />
                Send
              </Button>
            </div>
            {(connectMutation.error ||
              inputMutation.error ||
              resizeMutation.error ||
              closeMutation.error ||
              exportMutation.error) && (
              <InlineStatus className="mt-2" icon={<XCircle size={14} />} tone="danger">
                {formatError(
                  connectMutation.error ??
                    inputMutation.error ??
                    resizeMutation.error ??
                    closeMutation.error ??
                    exportMutation.error,
                )}
              </InlineStatus>
            )}
            {exportedLog && (
              <pre className="mt-3 max-h-32 overflow-auto rounded-md bg-slate-900 p-3 text-xs text-slate-300 ring-1 ring-inset ring-slate-800">
                {exportedLog}
              </pre>
            )}
          </div>
        </div>
      </Panel>
    </div>
  );
}

function DatabasePanel({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const {
    selectedDatabaseConnectionId: selectedConnectionId,
    setSelectedDatabaseConnection,
  } = useWorkspaceStore();
  const [testResult, setTestResult] = useState<DatabaseTestResult | null>(null);
  const [queryResult, setQueryResult] = useState<DatabaseQueryResult | null>(null);
  const [pendingSqlConfirmation, setPendingSqlConfirmation] = useState(false);
  const [sql, setSql] = useState(
    "select name, type\nfrom sqlite_master\nwhere type in ('table', 'view')\nlimit 100;",
  );
  const [tableView, setTableView] = useState<DatabaseTableViewState | null>(null);
  const [resultMode, setResultMode] = useState<"sql" | "table">("sql");
  const [form, setForm] = useState<DatabaseConnectionInput>({
    workspaceId,
    name: "Local SQLite",
    driver: "sqlite",
    sqlitePath: "",
  });

  const connectionsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["database-connections", workspaceId],
    queryFn: () => listDatabaseConnections(workspaceId),
  });

  const selectedConnection: DatabaseConnection | null =
    connectionsQuery.data?.find((item) => item.id === selectedConnectionId) ?? null;

  const schemaQuery = useQuery({
    enabled: Boolean(workspaceId && selectedConnectionId && selectedConnection?.driver === "sqlite"),
    queryKey: ["database-schema", workspaceId, selectedConnectionId],
    queryFn: () => getDatabaseSchema(workspaceId, selectedConnectionId ?? ""),
  });

  useEffect(() => {
    if (!connectionsQuery.data?.length) {
      setSelectedDatabaseConnection(null);
      return;
    }

    if (
      !selectedConnectionId ||
      !connectionsQuery.data.some((connection) => connection.id === selectedConnectionId)
    ) {
      setSelectedDatabaseConnection(connectionsQuery.data[0].id);
    }
  }, [connectionsQuery.data, selectedConnectionId, setSelectedDatabaseConnection]);

  useEffect(() => {
    setForm((current) => ({ ...current, workspaceId }));
  }, [workspaceId]);

  useEffect(() => {
    if (!selectedConnection) {
      return;
    }

    setForm({
      id: selectedConnection.id,
      workspaceId,
      name: selectedConnection.name,
      driver: selectedConnection.driver,
      host: selectedConnection.host,
      port: selectedConnection.port,
      database: selectedConnection.database,
      username: selectedConnection.username,
      sqlitePath: selectedConnection.sqlitePath,
      credentialRef: selectedConnection.credentialRef,
    });
    setTestResult(null);
    setQueryResult(null);
    setTableView(null);
    setResultMode("sql");
    setPendingSqlConfirmation(false);
  }, [selectedConnection, workspaceId]);

  const saveMutation = useMutation({
    mutationFn: saveDatabaseConnection,
    onSuccess: (connection) => {
      setSelectedDatabaseConnection(connection.id);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteDatabaseConnection(workspaceId, connectionId),
    onSuccess: () => {
      setSelectedDatabaseConnection(null);
      setTestResult(null);
      setQueryResult(null);
      setTableView(null);
      setResultMode("sql");
      setPendingSqlConfirmation(false);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const testMutation = useMutation({
    mutationFn: (connectionId: string) => testDatabaseConnection(workspaceId, connectionId),
    onSuccess: (result) => {
      setTestResult(result);
      queryClient.invalidateQueries({
        queryKey: ["database-schema", workspaceId, selectedConnectionId],
      });
    },
  });

  const executeMutation = useMutation({
    onMutate: () => {
      setTableView(null);
      setResultMode("sql");
    },
    mutationFn: (confirmMutation: boolean) =>
      executeDatabaseQuery({
        workspaceId,
        connectionId: selectedConnectionId ?? "",
        sql,
        limit: 100,
        confirmMutation,
      }),
    onError: (error) => {
      setPendingSqlConfirmation(isConfirmationRequired(error));
    },
    onSuccess: (result) => {
      setPendingSqlConfirmation(false);
      setTableView(null);
      setQueryResult(result);
    },
  });

  const browseMutation = useMutation({
    onMutate: () => {
      setPendingSqlConfirmation(false);
      setResultMode("table");
    },
    mutationFn: ({
      pageIndex,
      pageSize,
      tableName,
    }: {
      pageIndex: number;
      pageSize: number;
      tableName: string;
    }) =>
      browseDatabaseTable({
        workspaceId,
        connectionId: selectedConnectionId ?? "",
        tableName,
        limit: pageSize,
        offset: pageIndex * pageSize,
    }),
    onSuccess: (browse) => {
      setPendingSqlConfirmation(false);
      setSql(browse.sql);
      setQueryResult(browse.result);
      setResultMode("table");
      setTableView({
        pageIndex: Math.floor(browse.offset / Math.max(1, browse.limit)),
        pageSize: browse.limit,
        readOnly: browse.readOnly,
        tableName: browse.tableName,
        totalRows: browse.totalRows,
      });
    },
  });

  function updateForm(patch: Partial<DatabaseConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate({
      ...form,
      credentialRef: form.credentialRef?.trim() || null,
      sqlitePath: form.sqlitePath?.trim() || null,
      host: form.host?.trim() || null,
      database: form.database?.trim() || null,
      username: form.username?.trim() || null,
    });
  }

  function newConnection() {
    setSelectedDatabaseConnection(null);
    setTestResult(null);
    setQueryResult(null);
    setTableView(null);
    setResultMode("sql");
    setPendingSqlConfirmation(false);
    setForm({
      workspaceId,
      name: "Local SQLite",
      driver: "sqlite",
      sqlitePath: "",
    });
  }

  function browseTablePage(tableName: string, pageIndex: number, pageSize: number) {
    browseMutation.mutate({
      pageIndex: Math.max(0, pageIndex),
      pageSize,
      tableName,
    });
  }

  function refreshTableView() {
    if (!tableView) {
      return;
    }
    browseTablePage(tableView.tableName, tableView.pageIndex, tableView.pageSize);
  }

  function changeTablePageSize(pageSize: number) {
    if (!tableView) {
      return;
    }
    browseTablePage(tableView.tableName, 0, pageSize);
  }

  const tableViewPageCount = tableView
    ? Math.max(1, Math.ceil(tableView.totalRows / tableView.pageSize))
    : 1;
  const tableViewStart = tableView?.totalRows
    ? tableView.pageIndex * tableView.pageSize + 1
    : 0;
  const tableViewEnd = tableView
    ? Math.min((tableView.pageIndex + 1) * tableView.pageSize, tableView.totalRows)
    : 0;

  return (
    <div className="grid h-full min-h-0 grid-cols-[280px_minmax(0,1fr)] gap-3">
      <Panel>
        <PanelHeader
          actions={
            <Button aria-label="New database connection" onClick={newConnection} size="icon" type="button" variant="ghost">
              <Plus size={15} />
            </Button>
          }
          icon={<Database size={16} />}
          title="Connections"
        />

        <form className="form-band space-y-3 border-b border-slate-200 p-3" onSubmit={submitConnection}>
          <FieldGroup title="Name">
            <Input
              onChange={(event) => updateForm({ name: event.target.value })}
              value={form.name}
            />
          </FieldGroup>
          <FieldGroup title="Driver">
            <select
              className="h-9 w-full rounded-md border border-slate-300 bg-white px-2 text-sm text-slate-950 shadow-xs outline-none transition-colors hover:border-slate-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-700/15"
              onChange={(event) =>
                updateForm({
                  driver: event.target.value as DatabaseConnectionInput["driver"],
                  sqlitePath: event.target.value === "sqlite" ? form.sqlitePath : null,
                  credentialRef: event.target.value === "sqlite" ? null : form.credentialRef,
                })
              }
              value={form.driver}
            >
              <option value="sqlite">SQLite</option>
              <option value="postgres">PostgreSQL</option>
              <option value="mysql">MySQL / MariaDB</option>
            </select>
          </FieldGroup>

          {form.driver === "sqlite" ? (
            <FieldGroup title="SQLite Path">
              <Input
                onChange={(event) => updateForm({ sqlitePath: event.target.value })}
                placeholder="E:\\data\\app.sqlite"
                value={form.sqlitePath ?? ""}
              />
            </FieldGroup>
          ) : (
            <div className="space-y-3">
              <div className="grid grid-cols-[1fr_84px] gap-2">
                <FieldGroup title="Host">
                  <Input
                    onChange={(event) => updateForm({ host: event.target.value })}
                    placeholder="127.0.0.1"
                    value={form.host ?? ""}
                  />
                </FieldGroup>
                <FieldGroup title="Port">
                  <Input
                    onChange={(event) =>
                      updateForm({
                        port: event.target.value ? Number(event.target.value) : null,
                      })
                    }
                    placeholder={form.driver === "postgres" ? "5432" : "3306"}
                    type="number"
                    value={form.port ?? ""}
                  />
                </FieldGroup>
              </div>
              <FieldGroup title="Database">
                <Input
                  onChange={(event) => updateForm({ database: event.target.value })}
                  value={form.database ?? ""}
                />
              </FieldGroup>
              <FieldGroup title="Username">
                <Input
                  onChange={(event) => updateForm({ username: event.target.value })}
                  value={form.username ?? ""}
                />
              </FieldGroup>
              <CredentialReferenceControl
                kind="database-password"
                label={`${form.name || "Database"} password`}
                onChange={(credentialRef) => updateForm({ credentialRef })}
                value={form.credentialRef}
                workspaceId={workspaceId}
              />
            </div>
          )}

          <div className="flex items-center gap-2">
            <Button disabled={saveMutation.isPending} type="submit">
              <Save size={15} />
              Save
            </Button>
            <Button
              disabled={!selectedConnectionId || testMutation.isPending}
              onClick={() => selectedConnectionId && testMutation.mutate(selectedConnectionId)}
              type="button"
              variant="outline"
            >
              <CheckCircle2 size={15} />
              Test
            </Button>
            <Button
              disabled={!selectedConnectionId || deleteMutation.isPending}
              onClick={() => selectedConnectionId && deleteMutation.mutate(selectedConnectionId)}
              size="icon"
              type="button"
              variant="ghost"
            >
              <Trash2 size={15} />
            </Button>
          </div>

          {(testResult || testMutation.error || saveMutation.error) && (
            <StatusLine
              error={testMutation.error ?? saveMutation.error}
              result={testResult}
            />
          )}
        </form>

        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
              Saved Connections
            </span>
            <Badge tone="neutral">{connectionsQuery.data?.length ?? 0}</Badge>
          </div>
          <div className="space-y-1">
            {connectionsQuery.data?.map((connection) => (
              <ResourceListItem
                key={connection.id}
                onClick={() => setSelectedDatabaseConnection(connection.id)}
                selected={selectedConnectionId === connection.id}
              >
                <span className="min-w-0 flex-1 truncate">{connection.name}</span>
                <Badge tone={connection.driver === "sqlite" ? "green" : "amber"}>
                  {connection.driver}
                </Badge>
              </ResourceListItem>
            ))}
            {connectionsQuery.data?.length === 0 && (
              <EmptyState>No database connections</EmptyState>
            )}
          </div>

          <div className="mt-4 border-t border-slate-200 pt-3">
            <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
              <Table2 size={14} />
              Schema
            </div>
            <SchemaTree
              disabled={!selectedConnectionId || browseMutation.isPending}
              error={schemaQuery.error}
              loading={schemaQuery.isFetching}
              onBrowse={(table) => browseTablePage(table.name, 0, tableView?.pageSize ?? 100)}
              schema={schemaQuery.data}
            />
          </div>
        </div>
      </Panel>
      <Panel>
        <PanelHeader
          actions={
            <>
              {selectedConnection && <Badge tone="neutral">{selectedConnection.name}</Badge>}
              {tableView && (
                <>
                  <Badge tone={tableView.readOnly ? "green" : "amber"}>read only</Badge>
                  <span className="text-xs text-slate-500">
                    {tableViewStart}-{tableViewEnd} of {tableView.totalRows}
                  </span>
                  <select
                    aria-label="Rows per page"
                    className="h-8 rounded-md border border-slate-300 bg-white px-2 text-xs text-slate-700 shadow-xs outline-none transition-colors hover:border-slate-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-700/15"
                    onChange={(event) => changeTablePageSize(Number(event.target.value))}
                    value={tableView.pageSize}
                  >
                    {[50, 100, 250, 500].map((pageSize) => (
                      <option key={pageSize} value={pageSize}>
                        {pageSize}
                      </option>
                    ))}
                  </select>
                  <Button
                    aria-label="Previous table page"
                    disabled={browseMutation.isPending || tableView.pageIndex <= 0}
                    onClick={() =>
                      browseTablePage(
                        tableView.tableName,
                        tableView.pageIndex - 1,
                        tableView.pageSize,
                      )
                    }
                    size="icon"
                    type="button"
                    variant="ghost"
                  >
                    <ChevronLeft size={15} />
                  </Button>
                  <Button
                    aria-label="Next table page"
                    disabled={
                      browseMutation.isPending ||
                      tableView.pageIndex >= tableViewPageCount - 1
                    }
                    onClick={() =>
                      browseTablePage(
                        tableView.tableName,
                        tableView.pageIndex + 1,
                        tableView.pageSize,
                      )
                    }
                    size="icon"
                    type="button"
                    variant="ghost"
                  >
                    <ChevronRight size={15} />
                  </Button>
                  <Button
                    aria-label="Refresh table data"
                    disabled={browseMutation.isPending}
                    onClick={refreshTableView}
                    size="icon"
                    type="button"
                    variant="ghost"
                  >
                    <RefreshCw size={15} />
                  </Button>
                </>
              )}
              <Button
                disabled={!selectedConnectionId || executeMutation.isPending}
                className={pendingSqlConfirmation ? "bg-rose-700 hover:bg-rose-800" : undefined}
                onClick={() => executeMutation.mutate(pendingSqlConfirmation)}
                size="sm"
                type="button"
              >
                <Play size={14} />
                {pendingSqlConfirmation ? "Confirm run" : "Run"}
              </Button>
            </>
          }
          icon={tableView ? <Table2 size={15} /> : <Clock size={15} />}
          subtitle={tableView ? tableView.tableName : undefined}
          title={tableView ? "Table Data" : "SQL Editor"}
        />
        <div className="min-h-0 flex-[0.55] border-b border-slate-200">
          <Editor
            defaultLanguage="sql"
            onChange={(value) => setSql(value ?? "")}
            options={{
              fontSize: 13,
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              wordWrap: "on",
            }}
            value={sql}
          />
        </div>
        <DatabaseResultView
          error={resultMode === "table" ? browseMutation.error : executeMutation.error}
          isPending={executeMutation.isPending || browseMutation.isPending}
          pendingConfirmation={pendingSqlConfirmation}
          result={queryResult}
        />
      </Panel>
    </div>
  );
}

function StatusLine({
  error,
  result,
}: {
  error: unknown;
  result: DatabaseTestResult | null;
}) {
  if (error) {
    return (
      <InlineStatus icon={<XCircle size={14} />} tone="danger">
        {formatError(error)}
      </InlineStatus>
    );
  }

  if (!result) {
    return null;
  }

  return (
    <InlineStatus
      icon={result.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
      tone={result.ok ? "success" : "warning"}
    >
      {result.message}
      {result.serverVersion ? ` (${result.serverVersion})` : ""}
    </InlineStatus>
  );
}

function SchemaTree({
  disabled,
  error,
  loading,
  onBrowse,
  schema,
}: {
  disabled: boolean;
  error: unknown;
  loading: boolean;
  onBrowse: (table: DatabaseTable) => void;
  schema?: DatabaseSchema;
}) {
  if (error) {
    return (
      <InlineStatus tone="warning">
        {formatError(error)}
      </InlineStatus>
    );
  }

  if (loading) {
    return <EmptyState className="p-3 text-xs">Loading schema...</EmptyState>;
  }

  if (!schema?.tables.length) {
    return (
      <EmptyState className="p-3 text-xs">
        Select a SQLite connection to inspect tables.
      </EmptyState>
    );
  }

  return (
    <div className="space-y-3">
      {schema.tables.map((table) => (
        <div key={table.name}>
          <div className="flex items-center justify-between gap-2 rounded-md bg-slate-50 px-2 py-1 text-xs font-semibold ring-1 ring-inset ring-slate-200">
            <span className="truncate">{table.name}</span>
            <div className="flex items-center gap-1">
              <Badge tone="neutral">{table.kind}</Badge>
              <Button
                disabled={disabled}
                onClick={() => onBrowse(table)}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Table2 size={13} />
              </Button>
            </div>
          </div>
          <div className="mt-1 space-y-1 pl-2">
            {table.columns.map((column) => (
              <div className="flex items-center gap-2 text-xs text-slate-600" key={column.name}>
                <span className="min-w-0 flex-1 truncate">{column.name}</span>
                <span className="text-slate-400">{column.dataType || "ANY"}</span>
                {column.primaryKey && <Badge tone="teal">pk</Badge>}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

type DatabaseTableViewState = {
  pageIndex: number;
  pageSize: number;
  readOnly: boolean;
  tableName: string;
  totalRows: number;
};

function DatabaseResultView({
  error,
  isPending,
  pendingConfirmation,
  result,
}: {
  error: unknown;
  isPending: boolean;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
}) {
  const pageSize = 250;
  const [pageIndex, setPageIndex] = useState(0);
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "failed">("idle");
  const [scrollTop, setScrollTop] = useState(0);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setPageIndex(0);
    setCopyStatus("idle");
    setScrollTop(0);
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  }, [result]);

  if (error) {
    return (
      <EmptyState
        className={cn(
          "min-h-0 flex-1 p-4",
          pendingConfirmation ? "text-amber-800" : "text-rose-800",
        )}
      >
        {pendingConfirmation ? confirmationMessage(error) : formatError(error)}
      </EmptyState>
    );
  }

  if (isPending) {
    return <EmptyState className="min-h-0 flex-1">Running query...</EmptyState>;
  }

  if (!result) {
    return <EmptyState className="min-h-0 flex-1">Query results will appear here.</EmptyState>;
  }

  if (result.columns.length === 0) {
    return (
      <EmptyState className="min-h-0 flex-1 text-slate-600">
        {result.affectedRows} rows affected in {result.durationMs}ms.
      </EmptyState>
    );
  }

  const queryResult = result;
  const pageCount = Math.max(1, Math.ceil(queryResult.rows.length / pageSize));
  const safePageIndex = Math.min(pageIndex, pageCount - 1);
  const startIndex = safePageIndex * pageSize;
  const pageRows = queryResult.rows.slice(startIndex, startIndex + pageSize);
  const displayStart = queryResult.rows.length ? startIndex + 1 : 0;
  const displayEnd = Math.min(startIndex + pageRows.length, queryResult.rows.length);
  const rowHeight = 33;
  const viewportHeight = scrollRef.current?.clientHeight ?? 420;
  const virtualized = pageRows.length > 80;
  const virtualStart = virtualized
    ? Math.max(0, Math.floor(scrollTop / rowHeight) - 8)
    : 0;
  const virtualEnd = virtualized
    ? Math.min(
        pageRows.length,
        virtualStart + Math.ceil(viewportHeight / rowHeight) + 16,
      )
    : pageRows.length;
  const visibleRows = pageRows.slice(virtualStart, virtualEnd);
  const topSpacerHeight = virtualized ? virtualStart * rowHeight : 0;
  const bottomSpacerHeight = virtualized
    ? Math.max(0, (pageRows.length - virtualEnd) * rowHeight)
    : 0;
  const columnWidths = queryResult.columns.map((column, columnIndex) =>
    queryResult.rows.reduce((width, row) => {
      const value = row[columnIndex] ?? "";
      return Math.min(Math.max(width, String(value).length * 8 + 48), 360);
    }, Math.min(Math.max(column.name.length * 9 + 72, 140), 260)),
  );

  async function copyTsv() {
    const text = serializeDatabaseResult(queryResult, "\t");
    try {
      await navigator.clipboard.writeText(text);
      setCopyStatus("copied");
      window.setTimeout(() => setCopyStatus("idle"), 1600);
    } catch {
      setCopyStatus("failed");
    }
  }

  function exportCsv() {
    const text = serializeDatabaseResult(queryResult, ",");
    const blob = new Blob([text], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `unfour-query-results-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.csv`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className="min-h-0 flex-1 overflow-auto"
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        ref={scrollRef}
      >
        <table className="data-table w-max min-w-full table-fixed text-left text-xs">
          <colgroup>
            {columnWidths.map((width, index) => (
              <col key={`db-col-${index}`} style={{ width }} />
            ))}
          </colgroup>
          <thead className="sticky top-0">
            <tr>
              {queryResult.columns.map((column) => (
                <th className="border-b border-slate-200 px-3 py-2 font-medium" key={column.name}>
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate">{column.name}</span>
                    <span className="shrink-0 text-[10px] uppercase text-slate-400">
                      {column.dataType}
                    </span>
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {topSpacerHeight > 0 && (
              <tr aria-hidden="true">
                <td colSpan={queryResult.columns.length} style={{ height: topSpacerHeight }} />
              </tr>
            )}
            {visibleRows.map((row, rowIndex) => (
              <tr
                className="border-b"
                key={`db-row-${startIndex + virtualStart + rowIndex}`}
              >
                {row.map((value, cellIndex) => (
                  <td className="truncate px-3 py-2" key={`db-cell-${cellIndex}`}>
                    {value ?? <span className="text-slate-400">NULL</span>}
                  </td>
                ))}
              </tr>
            ))}
            {bottomSpacerHeight > 0 && (
              <tr aria-hidden="true">
                <td
                  colSpan={queryResult.columns.length}
                  style={{ height: bottomSpacerHeight }}
                />
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex h-10 items-center justify-between gap-3 border-t border-slate-200 bg-slate-50 px-3 text-xs text-slate-500">
        <span>
          {displayStart}-{displayEnd} of {queryResult.rows.length} rows in{" "}
          {queryResult.durationMs}ms
        </span>
        <div className="flex shrink-0 items-center gap-2">
          <Button onClick={copyTsv} size="sm" type="button" variant="outline">
            <Clipboard size={13} />
            {copyStatus === "copied"
              ? "Copied"
              : copyStatus === "failed"
                ? "Copy failed"
                : "Copy TSV"}
          </Button>
          <Button onClick={exportCsv} size="sm" type="button" variant="outline">
            <Download size={13} />
            Export CSV
          </Button>
          <Button
            disabled={safePageIndex === 0}
            onClick={() => {
              setPageIndex((current) => Math.max(0, current - 1));
              setScrollTop(0);
              if (scrollRef.current) {
                scrollRef.current.scrollTop = 0;
              }
            }}
            size="sm"
            type="button"
            variant="outline"
          >
            Prev
          </Button>
          <span>
            Page {safePageIndex + 1} / {pageCount}
          </span>
          <Button
            disabled={safePageIndex >= pageCount - 1}
            onClick={() => {
              setPageIndex((current) => Math.min(pageCount - 1, current + 1));
              setScrollTop(0);
              if (scrollRef.current) {
                scrollRef.current.scrollTop = 0;
              }
            }}
            size="sm"
            type="button"
            variant="outline"
          >
            Next
          </Button>
        </div>
      </div>
    </div>
  );
}

function serializeDatabaseResult(result: DatabaseQueryResult, delimiter: "," | "\t") {
  const header = result.columns
    .map((column) => serializeCell(column.name, delimiter))
    .join(delimiter);
  const rows = result.rows.map((row) =>
    result.columns
      .map((_, index) => serializeCell(row[index] ?? "", delimiter))
      .join(delimiter),
  );
  return [header, ...rows].join("\r\n");
}

function serializeCell(value: string, delimiter: "," | "\t") {
  const needsQuotes =
    value.includes(delimiter) ||
    value.includes("\"") ||
    value.includes("\n") ||
    value.includes("\r");
  if (!needsQuotes) {
    return value;
  }
  return `"${value.replace(/"/g, "\"\"")}"`;
}

function isConfirmationRequired(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code: unknown }).code === "CONFIRMATION_REQUIRED"
  );
}

function confirmationMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "details" in error) {
    const details = (error as { details?: { classification?: unknown } }).details;
    if (details?.classification) {
      return `Confirmation required for ${String(details.classification)} SQL. Review the statement, then click Confirm run.`;
    }
  }
  return "Confirmation required. Review the SQL statement, then click Confirm run.";
}

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

function formatResponseBody(body?: string) {
  if (!body) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

export default App;
