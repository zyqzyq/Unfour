import Editor from "@monaco-editor/react";
import {
  Activity,
  Braces,
  CheckCircle2,
  ChevronLeft,
  Clock,
  Clipboard,
  Database,
  Download,
  Folder,
  Globe2,
  History,
  KeyRound,
  Pencil,
  Play,
  Plus,
  Save,
  Send,
  Server,
  Settings,
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
  deleteDatabaseConnection,
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
  renameWorkspace,
  saveApiRequest,
  saveDatabaseConnection,
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
} from "./types";

const methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];

function App() {
  const queryClient = useQueryClient();
  const {
    activeTabId,
    activeWorkspaceId,
    hydrateLayout,
    layoutWorkspaceId,
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
    <div className="flex h-screen min-h-[680px] bg-zinc-100 text-zinc-950">
      <aside
        className={cn(
          "flex h-full shrink-0 flex-col border-r border-zinc-200 bg-white transition-all",
          sidebarCollapsed ? "w-[64px]" : "w-[292px]",
        )}
      >
        <div className="flex h-14 items-center gap-2 border-b border-zinc-200 px-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-md bg-zinc-950 text-white">
            <Activity size={17} />
          </div>
          {!sidebarCollapsed && (
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold">Unfour Workspace</div>
              <div className="truncate text-xs text-zinc-500">
                {healthQuery.data?.syncStrategy ?? "local-first"}
              </div>
            </div>
          )}
          <Button
            aria-label="Toggle sidebar"
            className="ml-auto"
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
              className="flex gap-2"
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
                  "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm",
                  activeWorkspace?.id === workspace.id
                    ? "bg-teal-50 text-teal-800"
                    : "text-zinc-700 hover:bg-zinc-100",
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

        <div className="border-t border-zinc-200 p-3">
          <SidebarAction
            collapsed={sidebarCollapsed}
            icon={<Settings size={15} />}
            label="Settings"
            onClick={() => setActiveTab("api-main")}
            selected={false}
          />
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center justify-between border-b border-zinc-200 bg-white px-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Input
                className="h-8 w-[260px] border-transparent bg-zinc-50 font-semibold"
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
            <div className="flex items-center gap-2 text-xs text-zinc-500">
              <Badge tone={healthQuery.data?.storageReady ? "green" : "amber"}>
                {healthQuery.data?.storageReady ? "local storage" : "checking"}
              </Badge>
              <span>{healthQuery.data?.aiReservedCapabilities.length ?? 0} AI hooks reserved</span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge tone="neutral">offline-first</Badge>
            <Badge tone="teal">command bus</Badge>
          </div>
        </header>

        <div className="flex h-10 items-end gap-1 border-b border-zinc-200 bg-white px-3">
          {tabs.map((tab) => (
            <button
              className={cn(
                "flex h-8 items-center gap-2 rounded-t-md border border-b-0 px-3 text-sm",
                activeTabId === tab.id
                  ? "border-zinc-200 bg-zinc-100 text-zinc-950"
                  : "border-transparent text-zinc-500 hover:bg-zinc-50",
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

        <section className="min-h-0 flex-1 overflow-hidden p-4">
          {activeTab.kind === "api" && activeWorkspace && (
            <ApiClientPanel workspaceId={activeWorkspace.id} />
          )}
          {activeTab.kind === "ssh" && <SshPanel />}
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
      <div className="mb-1 flex h-7 items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wide text-zinc-500">
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
        "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-sm",
        selected ? "bg-zinc-950 text-white" : "text-zinc-700 hover:bg-zinc-100",
      )}
      onClick={onClick}
      type="button"
    >
      {icon}
      {!collapsed && <span className="truncate">{label}</span>}
    </button>
  );
}

function ApiClientPanel({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const [method, setMethod] = useState("GET");
  const [url, setUrl] = useState("{{base_url}}/get");
  const [name, setName] = useState("Health check");
  const [headers, setHeaders] = useState<KeyValue[]>([
    { key: "Accept", value: "application/json", enabled: true },
  ]);
  const [query, setQuery] = useState<KeyValue[]>([
    { key: "source", value: "{{source}}", enabled: true },
  ]);
  const [body, setBody] = useState("{\n  \"hello\": \"workspace\"\n}");
  const [envVariables, setEnvVariables] = useState<KeyValue[]>([]);
  const [collectionStatus, setCollectionStatus] = useState("");
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

  const input = useMemo<ApiRequestInput>(
    () => ({
      workspaceId,
      name,
      method,
      url,
      headers,
      query,
      body: method === "GET" || method === "HEAD" ? undefined : body,
      bodyKind: "json",
      timeoutMs: 60_000,
    }),
    [body, headers, method, name, query, url, workspaceId],
  );

  const sendMutation = useMutation({
    mutationFn: sendApiRequest,
    onSuccess: (result) => {
      setResponse(result);
      queryClient.invalidateQueries({ queryKey: ["api-history", workspaceId] });
    },
  });

  const saveMutation = useMutation({
    mutationFn: saveApiRequest,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] }),
  });

  const saveEnvironmentMutation = useMutation({
    mutationFn: (variables: KeyValue[]) => updateWorkspaceEnvironment(workspaceId, variables),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["workspace-environment", workspaceId] }),
  });

  const replayHistoryMutation = useMutation({
    mutationFn: (historyId: string) => getApiHistoryDetail(workspaceId, historyId),
    onSuccess: loadHistoryRequest,
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
    setMethod(request.method);
    setUrl(request.url);
    setHeaders(request.headers);
    setQuery(request.query);
    setBody(request.body ?? "");
  }

  function loadSavedRequest(saved: ApiSavedRequest) {
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
    <div className="grid h-full min-h-0 grid-cols-[minmax(520px,1fr)_380px] gap-4">
      <form className="flex min-h-0 flex-col rounded-md border border-zinc-200 bg-white" onSubmit={submit}>
        <div className="flex items-center gap-2 border-b border-zinc-200 p-3">
          <select
            className="h-9 rounded-md border border-zinc-200 bg-white px-2 text-sm font-semibold outline-none focus:border-teal-500"
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
        </div>

        <div className="grid min-h-0 flex-1 grid-cols-[320px_minmax(0,1fr)]">
          <div className="space-y-4 overflow-y-auto border-r border-zinc-200 p-3">
            <SavedRequestsList
              collectionStatus={collectionStatus}
              importing={importCollectionMutation.isPending}
              items={savedQuery.data ?? []}
              onExport={exportCollection}
              onImport={importCollection}
              onLoad={loadSavedRequest}
            />
            <FieldGroup title="Request">
              <Input onChange={(event) => setName(event.target.value)} value={name} />
            </FieldGroup>
            <div className="rounded-md border border-zinc-200 p-3">
              <div className="mb-2 flex items-center justify-between">
                <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
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
            <KeyValueEditor items={query} onChange={setQuery} title="Query" />
            <KeyValueEditor items={headers} onChange={setHeaders} title="Headers" />
          </div>

          <div className="flex min-h-0 flex-col">
            <div className="flex h-10 items-center border-b border-zinc-200 px-3 text-sm font-medium">
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
      </form>

      <div className="flex min-h-0 flex-col gap-4">
        <section className="flex min-h-0 flex-1 flex-col rounded-md border border-zinc-200 bg-white">
          <div className="flex h-10 items-center justify-between border-b border-zinc-200 px-3">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Braces size={15} />
              Response
            </div>
            {response && (
              <div className="flex items-center gap-2">
                <Badge tone={response.status < 400 ? "green" : "red"}>
                  {response.status}
                </Badge>
                <Badge tone="neutral">{response.durationMs}ms</Badge>
              </div>
            )}
          </div>
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
        </section>

        <section className="h-[270px] rounded-md border border-zinc-200 bg-white">
          <div className="flex h-10 items-center justify-between border-b border-zinc-200 px-3">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <History size={15} />
              History
            </div>
            <Badge tone="neutral">{savedQuery.data?.length ?? 0} saved</Badge>
          </div>
          <HistoryTable
            items={historyQuery.data ?? []}
            loadingReplay={replayHistoryMutation.isPending}
            onReplay={(item) => replayHistoryMutation.mutate(item.id)}
          />
        </section>
      </div>
    </div>
  );
}

function SavedRequestsList({
  collectionStatus,
  importing,
  items,
  onExport,
  onImport,
  onLoad,
}: {
  collectionStatus: string;
  importing: boolean;
  items: ApiSavedRequest[];
  onExport: () => void;
  onImport: (file: File | undefined) => void;
  onLoad: (item: ApiSavedRequest) => void;
}) {
  return (
    <div className="rounded-md border border-zinc-200 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
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
              "inline-flex h-8 w-full cursor-pointer items-center justify-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-medium text-zinc-800 hover:bg-zinc-50",
              importing && "pointer-events-none opacity-50",
            )}
          >
            <Upload size={13} />
            Import
          </span>
        </label>
      </div>
      {collectionStatus && (
        <div className="mb-2 truncate rounded-md bg-zinc-50 px-2 py-1 text-xs text-zinc-600">
          {collectionStatus}
        </div>
      )}
      <div className="max-h-32 space-y-1 overflow-y-auto">
        {items.map((item) => (
          <button
            className="flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-xs hover:bg-zinc-100"
            key={item.id}
            onClick={() => onLoad(item)}
            type="button"
          >
            <Badge tone="teal">{item.method}</Badge>
            <span className="min-w-0 flex-1 truncate">{item.name}</span>
          </button>
        ))}
        {items.length === 0 && (
          <div className="py-3 text-center text-xs text-zinc-500">No saved requests</div>
        )}
      </div>
    </div>
  );
}

function FieldGroup({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <label className="block space-y-2">
      <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">{title}</span>
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
        <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">{title}</span>
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
        <div className="rounded-md bg-amber-50 px-2 py-1 text-amber-700">
          Duplicate variables: {duplicateKeys.join(", ")}
        </div>
      )}
      {sensitiveKeys.length > 0 && (
        <div className="rounded-md bg-zinc-50 px-2 py-1 text-zinc-600">
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
      <div className="grid grid-cols-3 border-b border-zinc-200 text-xs text-zinc-500">
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
    <div className="grid grid-cols-3 border-b border-zinc-200 text-xs text-zinc-600">
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-zinc-800">{response.headers.length}</span> headers
        {cookies.length > 0 && <span>, {cookies.length} cookies</span>}
      </div>
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-zinc-800">{bodySize}</span> body, {headerSize} headers
      </div>
      <div className="min-w-0 px-3 py-2">
        <span className="font-medium text-zinc-800">{response.durationMs}ms</span> total
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
    <div className="grid max-h-32 grid-cols-[1.4fr_1fr] overflow-hidden border-b border-zinc-200 text-xs">
      <div className="min-w-0 overflow-auto border-r border-zinc-200 p-2">
        <div className="mb-1 font-semibold uppercase tracking-wide text-zinc-500">Headers</div>
        <div className="space-y-1">
          {response.headers.map((header) => (
            <div className="grid grid-cols-[120px_minmax(0,1fr)] gap-2" key={`${header.key}-${header.value}`}>
              <span className="truncate font-medium text-zinc-700">{header.key}</span>
              <span className="truncate text-zinc-500">{header.value}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="min-w-0 overflow-auto p-2">
        <div className="mb-1 font-semibold uppercase tracking-wide text-zinc-500">Timing / Size</div>
        <div className="grid grid-cols-2 gap-2 text-zinc-600">
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
    <div className="min-w-0 rounded-md bg-zinc-50 px-2 py-1">
      <div className="text-[10px] uppercase text-zinc-400">{label}</div>
      <div className="truncate font-medium text-zinc-700">{value}</div>
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
    <div className="h-[228px] overflow-auto">
      <table className="w-full text-left text-xs">
        <thead className="sticky top-0 bg-zinc-50 text-zinc-500">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <th className="border-b border-zinc-200 px-3 py-2 font-medium" key={header.id}>
                  {flexRender(header.column.columnDef.header, header.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr className="border-b border-zinc-100" key={row.id}>
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
        <div className="flex h-24 items-center justify-center text-sm text-zinc-500">
          No requests yet
        </div>
      )}
    </div>
  );
}

function SshPanel() {
  return (
    <div className="grid h-full min-h-0 grid-cols-[360px_minmax(0,1fr)] gap-4">
      <section className="rounded-md border border-zinc-200 bg-white p-4">
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold">
          <TerminalSquare size={16} />
          SSH Sessions
        </div>
        <div className="space-y-3">
          <Input placeholder="Host" value="example.internal" readOnly />
          <Input placeholder="User" value="deploy" readOnly />
          <div className="grid grid-cols-2 gap-2">
            <Button type="button" variant="outline">
              <KeyRound size={15} />
              Password
            </Button>
            <Button type="button" variant="outline">
              <KeyRound size={15} />
              Private key
            </Button>
          </div>
          <Badge tone="amber">reserved backend</Badge>
        </div>
      </section>
      <TerminalPreview />
    </div>
  );
}

function DatabasePanel({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const [selectedConnectionId, setSelectedConnectionId] = useState<string | null>(null);
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
      setSelectedConnectionId(null);
      return;
    }

    if (!selectedConnectionId) {
      setSelectedConnectionId(connectionsQuery.data[0].id);
    }
  }, [connectionsQuery.data, selectedConnectionId]);

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
      setSelectedConnectionId(connection.id);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteDatabaseConnection(workspaceId, connectionId),
    onSuccess: () => {
      setSelectedConnectionId(null);
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
    setSelectedConnectionId(null);
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
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] gap-4">
      <section className="flex min-h-0 flex-col rounded-md border border-zinc-200 bg-white">
        <div className="flex h-10 items-center justify-between border-b border-zinc-200 px-3">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Database size={16} />
            Connections
          </div>
          <Button onClick={newConnection} size="icon" type="button" variant="ghost">
            <Plus size={15} />
          </Button>
        </div>

        <form className="space-y-3 border-b border-zinc-200 p-3" onSubmit={submitConnection}>
          <FieldGroup title="Name">
            <Input
              onChange={(event) => updateForm({ name: event.target.value })}
              value={form.name}
            />
          </FieldGroup>
          <FieldGroup title="Driver">
            <select
              className="h-9 w-full rounded-md border border-zinc-200 bg-white px-2 text-sm outline-none focus:border-teal-500"
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
            <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              Saved Connections
            </span>
            <Badge tone="neutral">{connectionsQuery.data?.length ?? 0}</Badge>
          </div>
          <div className="space-y-1">
            {connectionsQuery.data?.map((connection) => (
              <button
                className={cn(
                  "flex min-h-9 w-full items-center justify-between gap-2 rounded-md px-2 text-left text-sm",
                  selectedConnectionId === connection.id
                    ? "bg-teal-50 text-teal-800"
                    : "hover:bg-zinc-100",
                )}
                key={connection.id}
                onClick={() => setSelectedConnectionId(connection.id)}
                type="button"
              >
                <span className="min-w-0 flex-1 truncate">{connection.name}</span>
                <Badge tone={connection.driver === "sqlite" ? "green" : "amber"}>
                  {connection.driver}
                </Badge>
              </button>
            ))}
            {connectionsQuery.data?.length === 0 && (
              <div className="py-4 text-center text-sm text-zinc-500">
                No database connections
              </div>
            )}
          </div>

          <div className="mt-4 border-t border-zinc-200 pt-3">
            <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
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
      </section>
      <section className="flex min-h-0 flex-col rounded-md border border-zinc-200 bg-white">
        <div className="flex h-10 items-center justify-between border-b border-zinc-200 px-3">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <Clock size={15} />
            SQL Editor
          </div>
          <div className="flex items-center gap-2">
            {selectedConnection && <Badge tone="neutral">{selectedConnection.name}</Badge>}
            <Button
              disabled={!selectedConnectionId || executeMutation.isPending}
              className={pendingSqlConfirmation ? "bg-red-700 hover:bg-red-800" : undefined}
              onClick={() => executeMutation.mutate(pendingSqlConfirmation)}
              size="sm"
              type="button"
            >
              <Play size={14} />
              {pendingSqlConfirmation ? "Confirm run" : "Run"}
            </Button>
          </div>
        </div>
        <div className="min-h-0 flex-[0.55] border-b border-zinc-200">
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
      </section>
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
      <div className="flex items-center gap-2 rounded-md bg-red-50 px-2 py-2 text-xs text-red-700">
        <XCircle size={14} />
        <span className="min-w-0 flex-1">{formatError(error)}</span>
      </div>
    );
  }

  if (!result) {
    return null;
  }

  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md px-2 py-2 text-xs",
        result.ok ? "bg-green-50 text-green-700" : "bg-amber-50 text-amber-700",
      )}
    >
      {result.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
      <span className="min-w-0 flex-1">
        {result.message}
        {result.serverVersion ? ` (${result.serverVersion})` : ""}
      </span>
    </div>
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
    return <div className="text-xs text-amber-700">{formatError(error)}</div>;
  }

  if (loading) {
    return <div className="text-xs text-zinc-500">Loading schema...</div>;
  }

  if (!schema?.tables.length) {
    return <div className="text-xs text-zinc-500">Select a SQLite connection to inspect tables.</div>;
  }

  return (
    <div className="space-y-3">
      {schema.tables.map((table) => (
        <div key={table.name}>
          <div className="flex items-center justify-between gap-2 rounded-md bg-zinc-50 px-2 py-1 text-xs font-semibold">
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
              <div className="flex items-center gap-2 text-xs text-zinc-600" key={column.name}>
                <span className="min-w-0 flex-1 truncate">{column.name}</span>
                <span className="text-zinc-400">{column.dataType || "ANY"}</span>
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
      <div
        className={cn(
          "flex min-h-0 flex-1 items-center justify-center p-4 text-sm",
          pendingConfirmation ? "text-amber-700" : "text-red-700",
        )}
      >
        {pendingConfirmation ? confirmationMessage(error) : formatError(error)}
      </div>
    );
  }

  if (isPending) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-zinc-500">
        Running query...
      </div>
    );
  }

  if (!result) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-zinc-500">
        Query results will appear here.
      </div>
    );
  }

  if (result.columns.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-zinc-600">
        {result.affectedRows} rows affected in {result.durationMs}ms.
      </div>
    );
  }

  const queryResult = result;
  const pageCount = Math.max(1, Math.ceil(queryResult.rows.length / pageSize));
  const safePageIndex = Math.min(pageIndex, pageCount - 1);
  const startIndex = safePageIndex * pageSize;
  const pageRows = queryResult.rows.slice(startIndex, startIndex + pageSize);
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
        <table className="w-max min-w-full table-fixed text-left text-xs">
          <colgroup>
            {columnWidths.map((width, index) => (
              <col key={`db-col-${index}`} style={{ width }} />
            ))}
          </colgroup>
          <thead className="sticky top-0 bg-zinc-50 text-zinc-500">
            <tr>
              {queryResult.columns.map((column) => (
                <th className="border-b border-zinc-200 px-3 py-2 font-medium" key={column.name}>
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate">{column.name}</span>
                    <span className="shrink-0 text-[10px] uppercase text-zinc-400">
                      {column.dataType}
                    </span>
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {pageRows.map((row, rowIndex) => (
              <tr className="border-b border-zinc-100" key={`db-row-${startIndex + rowIndex}`}>
                {row.map((value, cellIndex) => (
                  <td className="truncate px-3 py-2" key={`db-cell-${cellIndex}`}>
                    {value ?? <span className="text-zinc-400">NULL</span>}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="flex h-10 items-center justify-between gap-3 border-t border-zinc-200 px-3 text-xs text-zinc-500">
        <span>
          {startIndex + 1}-{Math.min(startIndex + pageRows.length, queryResult.rows.length)} of{" "}
          {queryResult.rows.length} rows in {queryResult.durationMs}ms
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
