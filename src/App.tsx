import Editor from "@monaco-editor/react";
import {
  Activity,
  Braces,
  CheckCircle2,
  ChevronLeft,
  Clock,
  Clipboard,
  Copy,
  Database,
  Download,
  Folder,
  Globe2,
  Pencil,
  Play,
  Plus,
  Save,
  Send,
  Server,
  Table2,
  TerminalSquare,
  Trash2,
  Upload,
  XCircle,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { TerminalPreview } from "./components/TerminalPreview";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import {
  browseDatabaseTable,
  createWorkspace,
  deleteApiRequest,
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
  renameWorkspace,
  saveApiRequest,
  duplicateApiRequest,
  saveDatabaseConnection,
  saveSshConnection,
  sendApiRequest,
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
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
  KeyValue,
  SshConnection,
  SshConnectionInput,
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
    sidebarCollapsed,
    snapshotLayout,
    toggleSidebar,
    tabs,
  } = useWorkspaceStore();
  const [newWorkspaceName, setNewWorkspaceName] = useState("");
  const [workspaceDraftName, setWorkspaceDraftName] = useState("");

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

  useEffect(() => {
    if (workspaceQuery.data?.activeWorkspaceId && !activeWorkspaceId) {
      setActiveWorkspace(workspaceQuery.data.activeWorkspaceId);
    }
  }, [activeWorkspaceId, setActiveWorkspace, workspaceQuery.data?.activeWorkspaceId]);

  useEffect(() => {
    if (activeWorkspace?.name) {
      setWorkspaceDraftName(activeWorkspace.name);
    }
  }, [activeWorkspace?.id, activeWorkspace?.name]);

  useEffect(() => {
    if (workspaceLayoutQuery.data) {
      hydrateLayout(workspaceLayoutQuery.data);
    }
  }, [hydrateLayout, workspaceLayoutQuery.data]);

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
      setNewWorkspaceName("");
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
    <div className="app-shell flex h-screen min-h-[680px] text-slate-950">
      <aside
        className={cn(
          "sidebar-shell flex h-full shrink-0 flex-col border-r transition-all duration-200",
          sidebarCollapsed ? "w-[64px]" : "w-[240px]",
        )}
      >
        <div className="sidebar-divider flex h-14 items-center gap-2 border-b px-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-md bg-teal-500 text-white shadow-sm shadow-teal-950/20">
            <Activity size={17} />
          </div>
          {!sidebarCollapsed && (
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold">Unfour Workspace</div>
              <div className="truncate text-xs text-slate-400">
                {healthQuery.data?.syncStrategy ?? "local-first"}
              </div>
            </div>
          )}
          <Button
            aria-label="Toggle sidebar"
            className="ml-auto text-slate-300 hover:bg-white/10 hover:text-white"
            onClick={toggleSidebar}
            size="icon"
            type="button"
            variant="ghost"
          >
            <ChevronLeft
              className={cn("transition-transform", sidebarCollapsed && "rotate-180")}
              size={17}
            />
          </Button>
        </div>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
          {!sidebarCollapsed && (
            <form
              className="workspace-card flex gap-2 rounded-md p-1.5"
              onSubmit={(event) => {
                event.preventDefault();
                if (newWorkspaceName.trim()) {
                  createWorkspaceMutation.mutate(newWorkspaceName);
                }
              }}
            >
              <Input
                onChange={(event) => setNewWorkspaceName(event.target.value)}
                placeholder="Workspace name"
                value={newWorkspaceName}
              />
              <Button disabled={createWorkspaceMutation.isPending} size="icon" type="submit">
                <Plus size={16} />
              </Button>
            </form>
          )}

          <ResourceGroup
            collapsed={sidebarCollapsed}
            icon={<Folder size={16} />}
            title="Workspaces"
          >
            {workspaceQuery.data?.workspaces.map((workspace) => (
              <button
                className={cn(
                  "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors",
                  activeWorkspace?.id === workspace.id
                    ? "bg-teal-500/20 text-white ring-1 ring-inset ring-teal-400/35"
                    : "text-slate-300 hover:bg-white/10 hover:text-white",
                )}
                key={workspace.id}
                onClick={() => activateWorkspaceMutation.mutate(workspace.id)}
                type="button"
              >
                <Folder size={14} />
                {!sidebarCollapsed && (
                  <>
                    <span className="min-w-0 flex-1 truncate">{workspace.name}</span>
                    {workspace.isDefault && <Badge tone="teal">default</Badge>}
                  </>
                )}
              </button>
            ))}
          </ResourceGroup>

          <ResourceGroup
            collapsed={sidebarCollapsed}
            icon={<Globe2 size={16} />}
            title="API Collections"
          >
            <SidebarAction
              collapsed={sidebarCollapsed}
              icon={<Braces size={14} />}
              label="REST Client"
              onClick={() => setActiveTab("api-main")}
              selected={activeTabId === "api-main"}
            />
          </ResourceGroup>

          <ResourceGroup
            collapsed={sidebarCollapsed}
            icon={<Server size={16} />}
            title="Connections"
          >
            <SidebarAction
              collapsed={sidebarCollapsed}
              icon={<TerminalSquare size={14} />}
              label="SSH Sessions"
              onClick={() => setActiveTab("ssh-main")}
              selected={activeTabId === "ssh-main"}
            />
            <SidebarAction
              collapsed={sidebarCollapsed}
              icon={<Database size={14} />}
              label="Databases"
              onClick={() => setActiveTab("database-main")}
              selected={activeTabId === "database-main"}
            />
          </ResourceGroup>
        </div>

        {!sidebarCollapsed && (
          <div className="sidebar-divider border-t px-3 py-2 text-xs text-slate-500">
            Local workspace
          </div>
        )}
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center justify-between border-b border-slate-300/80 bg-white/95 px-4 shadow-sm">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Input
                className="h-8 w-[260px] border-transparent bg-slate-100 font-semibold shadow-none hover:bg-white"
                onChange={(event) => setWorkspaceDraftName(event.target.value)}
                value={workspaceDraftName}
              />
              <Button
                disabled={
                  !activeWorkspace ||
                  renameWorkspaceMutation.isPending ||
                  workspaceDraftName.trim() === activeWorkspace.name
                }
                onClick={() =>
                  activeWorkspace &&
                  renameWorkspaceMutation.mutate({
                    workspaceId: activeWorkspace.id,
                    name: workspaceDraftName,
                  })
                }
                size="icon"
                type="button"
                variant="ghost"
              >
                <Pencil size={15} />
              </Button>
              <Button
                disabled={
                  !activeWorkspace ||
                  activeWorkspace.isDefault ||
                  deleteWorkspaceMutation.isPending ||
                  (workspaceQuery.data?.workspaces.length ?? 0) <= 1
                }
                onClick={() =>
                  activeWorkspace && deleteWorkspaceMutation.mutate(activeWorkspace.id)
                }
                size="icon"
                type="button"
                variant="ghost"
              >
                <Trash2 size={15} />
              </Button>
            </div>
            <div className="flex items-center gap-2 text-xs text-slate-500">
              <Badge tone={healthQuery.data?.storageReady ? "green" : "amber"}>
                {healthQuery.data?.storageReady ? "local storage" : "checking"}
              </Badge>
              <span>{healthQuery.data?.syncStrategy ?? "local-first"}</span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge tone="neutral">{activeTab.title}</Badge>
            <Badge tone="teal">offline-first</Badge>
          </div>
        </header>

        <div className="flex h-10 items-end gap-1 border-b border-slate-300/80 bg-slate-100/80 px-3">
          {tabs.map((tab) => (
            <button
              className={cn(
                "flex h-8 items-center gap-2 rounded-t-md border border-b-0 px-3 text-sm transition-colors duration-150",
                activeTabId === tab.id
                  ? "border-slate-300 bg-white text-slate-950 shadow-sm"
                  : "border-transparent text-slate-500 hover:bg-white/70 hover:text-slate-800",
              )}
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              type="button"
            >
              {tab.kind === "api" && <Globe2 size={14} />}
              {tab.kind === "ssh" && <TerminalSquare size={14} />}
              {tab.kind === "database" && <Database size={14} />}
              {tab.title}
            </button>
          ))}
        </div>

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
  );
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
      <div className="mb-1 flex h-7 items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wide text-slate-400">
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
          ? "bg-white text-slate-950 shadow-sm"
          : "text-slate-300 hover:bg-white/10 hover:text-white",
      )}
      onClick={onClick}
      type="button"
    >
      {icon}
      {!collapsed && <span className="truncate">{label}</span>}
    </button>
  );
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
  const [form, setForm] = useState<SshConnectionInput>({
    workspaceId,
    name: "Deploy host",
    host: "example.internal",
    port: 22,
    username: "deploy",
    authKind: "password",
    credentialRef: "credential://ssh/deploy-host",
  });

  const connectionsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["ssh-connections", workspaceId],
    queryFn: () => listSshConnections(workspaceId),
  });

  const selectedConnection: SshConnection | null =
    connectionsQuery.data?.find((item) => item.id === selectedConnectionId) ?? null;

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
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
    },
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
      credentialRef: "credential://ssh/deploy-host",
    });
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate(form);
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[280px_minmax(0,1fr)] gap-3">
      <Panel>
        <PanelHeader
          actions={
            <>
              <Badge tone="amber">reserved</Badge>
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
          <FieldGroup title="Credential Ref">
            <Input
              onChange={(event) => updateForm({ credentialRef: event.target.value })}
              placeholder="credential://ssh/deploy-host"
              value={form.credentialRef ?? ""}
            />
          </FieldGroup>

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
          <InlineStatus className="mt-3" tone="warning">
            Session login and terminal streaming remain reserved for the russh backend.
          </InlineStatus>
        </div>
      </Panel>
      <TerminalPreview />
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
      setQueryResult(result);
    },
  });

  const browseMutation = useMutation({
    mutationFn: (table: DatabaseTable) =>
      browseDatabaseTable({
        workspaceId,
        connectionId: selectedConnectionId ?? "",
        tableName: table.name,
        limit: 100,
    }),
    onSuccess: (browse) => {
      setPendingSqlConfirmation(false);
      setSql(browse.sql);
      setQueryResult(browse.result);
    },
  });

  function updateForm(patch: Partial<DatabaseConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate(form);
  }

  function newConnection() {
    setSelectedDatabaseConnection(null);
    setTestResult(null);
    setQueryResult(null);
    setPendingSqlConfirmation(false);
    setForm({
      workspaceId,
      name: "Local SQLite",
      driver: "sqlite",
      sqlitePath: "",
    });
  }

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
              onBrowse={(table) => browseMutation.mutate(table)}
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
          icon={<Clock size={15} />}
          title="SQL Editor"
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
          error={executeMutation.error}
          isPending={executeMutation.isPending}
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
  const pageSize = 25;
  const [pageIndex, setPageIndex] = useState(0);
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "failed">("idle");

  useEffect(() => {
    setPageIndex(0);
    setCopyStatus("idle");
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
      <div className="min-h-0 flex-1 overflow-auto">
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
            {pageRows.map((row, rowIndex) => (
              <tr className="border-b" key={`db-row-${startIndex + rowIndex}`}>
                {row.map((value, cellIndex) => (
                  <td className="truncate px-3 py-2" key={`db-cell-${cellIndex}`}>
                    {value ?? <span className="text-slate-400">NULL</span>}
                  </td>
                ))}
              </tr>
            ))}
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
            onClick={() => setPageIndex((current) => Math.max(0, current - 1))}
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
            onClick={() => setPageIndex((current) => Math.min(pageCount - 1, current + 1))}
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
