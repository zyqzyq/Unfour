import { invoke } from "@tauri-apps/api/core";
import type {
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
  CredentialCreateInput,
  CredentialDeleteInput,
  CredentialMetadata,
  DatabaseBrowseInput,
  DatabaseBrowseResult,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryInput,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTestResult,
  SshConnection,
  SshConnectionInput,
  SystemHealth,
  Workspace,
  WorkspaceEnvironment,
  WorkspaceLayout,
  WorkspaceState,
} from "../types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const mockWorkspace: Workspace = {
  id: "mock-workspace",
  name: "Default Workspace",
  isDefault: true,
  lastOpenedAt: new Date().toISOString(),
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  deletedAt: null,
  revision: 1,
  syncStatus: "local",
  remoteId: null,
};

const mockState: WorkspaceState = {
  activeWorkspaceId: mockWorkspace.id,
  workspaces: [mockWorkspace],
};

let mockHistory: ApiHistoryItem[] = [];
let mockHistoryDetails: ApiHistoryDetail[] = [];
let mockSavedRequests: ApiSavedRequest[] = [];
let mockDatabaseConnections: DatabaseConnection[] = [];
let mockSshConnections: SshConnection[] = [];
let mockCredentials: Record<string, string> = {};
let mockEnvironment: WorkspaceEnvironment = {
  workspaceId: mockWorkspace.id,
  variables: [
    { key: "base_url", value: "https://httpbin.org", enabled: true },
    { key: "source", value: "unfour", enabled: true },
  ],
  updatedAt: new Date().toISOString(),
};
let mockLayouts: Record<string, WorkspaceLayout> = {};

function isTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return mockInvoke<T>(command, args);
  }

  return invoke<T>(command, args);
}

async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (command === "system_health") {
    return {
      appName: "Unfour Workspace",
      storageReady: true,
      commandBusReady: true,
      aiReservedCapabilities: ["api.send_request", "ssh.connect.reserved"],
      syncStrategy: "local-first-reserved",
    } as T;
  }

  if (command === "workspace_list") {
    return mockState as T;
  }

  if (command === "workspace_create") {
    const workspace: Workspace = {
      ...mockWorkspace,
      id: crypto.randomUUID(),
      name: String(args?.name ?? "New Workspace"),
      isDefault: false,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    mockState.workspaces = [workspace, ...mockState.workspaces];
    mockState.activeWorkspaceId = workspace.id;
    return workspace as T;
  }

  if (command === "workspace_rename") {
    const workspaceId = String(args?.workspaceId ?? "");
    const workspace = mockState.workspaces.find((item) => item.id === workspaceId);
    if (!workspace) throw new Error("workspace not found");
    workspace.name = String(args?.name ?? workspace.name);
    workspace.updatedAt = new Date().toISOString();
    return workspace as T;
  }

  if (command === "workspace_delete") {
    const workspaceId = String(args?.workspaceId ?? "");
    if (mockState.workspaces.length <= 1) {
      throw new Error("at least one workspace must remain");
    }
    mockState.workspaces = mockState.workspaces.filter((item) => item.id !== workspaceId);
    if (mockState.activeWorkspaceId === workspaceId) {
      mockState.activeWorkspaceId = mockState.workspaces[0]?.id ?? mockWorkspace.id;
    }
    return mockState as T;
  }

  if (command === "workspace_set_active") {
    mockState.activeWorkspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockState as T;
  }

  if (command === "workspace_environment_get") {
    return {
      ...mockEnvironment,
      workspaceId: String(args?.workspaceId ?? mockState.activeWorkspaceId),
    } as T;
  }

  if (command === "workspace_environment_update") {
    mockEnvironment = {
      workspaceId: String(args?.workspaceId ?? mockState.activeWorkspaceId),
      variables: (args?.variables as WorkspaceEnvironment["variables"]) ?? [],
      updatedAt: new Date().toISOString(),
    };
    return mockEnvironment as T;
  }

  if (command === "workspace_layout_get") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return getMockLayout(workspaceId) as T;
  }

  if (command === "workspace_layout_update") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const layout = args?.layout as WorkspaceLayout;
    mockLayouts[workspaceId] = {
      ...layout,
      workspaceId,
      updatedAt: new Date().toISOString(),
    };
    return mockLayouts[workspaceId] as T;
  }

  if (command === "api_history_list") {
    return mockHistory as T;
  }

  if (command === "api_history_detail") {
    const workspaceId = String(args?.workspaceId ?? "");
    const historyId = String(args?.historyId ?? "");
    const detail = mockHistoryDetails.find(
      (item) => item.workspaceId === workspaceId && item.id === historyId,
    );
    if (!detail) throw new Error("api history not found");
    return detail as T;
  }

  if (command === "api_saved_requests") {
    return mockSavedRequests as T;
  }

  if (command === "api_request_save") {
    const input = args?.input as ApiRequestInput;
    const saved: ApiSavedRequest = {
      id: crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name || `${input.method} ${input.url}`,
      folderPath: normalizeFolderPath(input.folderPath),
      method: input.method,
      url: input.url,
      headersJson: JSON.stringify(input.headers),
      queryJson: JSON.stringify(input.query),
      body: input.body ?? null,
      bodyKind: input.bodyKind,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      deletedAt: null,
      revision: 1,
      syncStatus: "local",
      remoteId: null,
    };
    mockSavedRequests = [saved, ...mockSavedRequests];
    return saved as T;
  }

  if (command === "api_request_duplicate") {
    const workspaceId = String(args?.workspaceId ?? "");
    const requestId = String(args?.requestId ?? "");
    const source = mockSavedRequests.find(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (!source) throw new Error("api request not found");
    const now = new Date().toISOString();
    const duplicate: ApiSavedRequest = {
      ...source,
      id: crypto.randomUUID(),
      name: `${source.name} Copy`,
      createdAt: now,
      updatedAt: now,
      revision: 1,
      syncStatus: "local",
      remoteId: null,
    };
    mockSavedRequests = [duplicate, ...mockSavedRequests];
    return duplicate as T;
  }

  if (command === "api_request_delete") {
    const workspaceId = String(args?.workspaceId ?? "");
    const requestId = String(args?.requestId ?? "");
    const initialLength = mockSavedRequests.length;
    mockSavedRequests = mockSavedRequests.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === requestId),
    );
    if (mockSavedRequests.length === initialLength) throw new Error("api request not found");
    return mockSavedRequests.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "api_send_request") {
    const input = args?.input as ApiRequestInput;
    const started = performance.now();
    const resolved = resolveInput(input, mockEnvironment.variables);
    const url = new URL(resolved.url);
    resolved.query
      .filter((item) => item.enabled && item.key)
      .forEach((item) => url.searchParams.append(item.key, item.value));
    const headers = Object.fromEntries(
      resolved.headers
        .filter((item) => item.enabled && item.key)
        .map((item) => [item.key, item.value]),
    );
    const response = await fetch(url, {
      method: resolved.method,
      headers,
      body:
        resolved.method === "GET" || resolved.method === "HEAD"
          ? undefined
          : resolved.body || undefined,
    });
    const body = await response.text();
    const result: ApiResponse = {
      historyId: crypto.randomUUID(),
      status: response.status,
      statusText: response.statusText,
      headers: Array.from(response.headers.entries()).map(([key, value]) => ({
        key,
        value,
        enabled: true,
      })),
      body,
      durationMs: Math.round(performance.now() - started),
    };
    mockHistory = [
      {
        id: result.historyId,
        workspaceId: input.workspaceId,
        name: input.name ?? null,
        method: resolved.method,
        url: resolved.url,
        status: result.status,
        durationMs: result.durationMs,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        deletedAt: null,
        revision: 1,
        syncStatus: "local",
        remoteId: null,
      },
      ...mockHistory,
    ];
    mockHistoryDetails = [
      {
        id: result.historyId,
        workspaceId: input.workspaceId,
        name: input.name ?? null,
        method: resolved.method,
        url: resolved.url,
        requestHeadersJson: JSON.stringify(redactHeaders(input.headers)),
        requestQueryJson: JSON.stringify(input.query),
        requestBody: input.body ?? null,
        status: result.status,
        durationMs: result.durationMs,
        responseHeadersJson: JSON.stringify(result.headers),
        responseBodyPreview: body.slice(0, 20_000),
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        deletedAt: null,
        revision: 1,
        syncStatus: "local",
        remoteId: null,
      },
      ...mockHistoryDetails,
    ];
    return result as T;
  }

  if (command === "credential_create") {
    const input = args?.input as CredentialCreateInput;
    const credentialRef = `unfour-workspace:${input.workspaceId}:${input.kind}:${crypto.randomUUID()}`;
    mockCredentials[credentialRef] = input.secret;
    return ({
      workspaceId: input.workspaceId,
      kind: input.kind,
      label: input.label,
      credentialRef,
    } satisfies CredentialMetadata) as T;
  }

  if (command === "credential_delete") {
    const input = args?.input as CredentialDeleteInput;
    delete mockCredentials[input.credentialRef];
    return undefined as T;
  }

  if (command === "database_connections_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockDatabaseConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_connection_save") {
    const input = args?.input as DatabaseConnectionInput;
    const now = new Date().toISOString();
    const existingIndex = input.id
      ? mockDatabaseConnections.findIndex((item) => item.id === input.id)
      : -1;
    const connection: DatabaseConnection = {
      id: input.id || crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name,
      driver: input.driver,
      host: input.host ?? null,
      port: input.port ?? null,
      database: input.database ?? null,
      username: input.username ?? null,
      sqlitePath: input.sqlitePath ?? null,
      credentialRef: input.credentialRef ?? null,
      createdAt:
        existingIndex >= 0 ? mockDatabaseConnections[existingIndex].createdAt : now,
      updatedAt: now,
      deletedAt: null,
      revision: existingIndex >= 0 ? mockDatabaseConnections[existingIndex].revision + 1 : 1,
      syncStatus: existingIndex >= 0 ? "pending" : "local",
      remoteId: null,
    };
    if (existingIndex >= 0) {
      mockDatabaseConnections[existingIndex] = connection;
    } else {
      mockDatabaseConnections = [connection, ...mockDatabaseConnections];
    }
    return connection as T;
  }

  if (command === "database_connection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const connectionId = String(args?.connectionId ?? "");
    mockDatabaseConnections = mockDatabaseConnections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === connectionId),
    );
    return mockDatabaseConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_connection_test") {
    const connectionId = String(args?.connectionId ?? "");
    const connection = mockDatabaseConnections.find((item) => item.id === connectionId);
    return ({
      ok: connection?.driver === "sqlite",
      message:
        connection?.driver === "sqlite"
          ? "SQLite connection OK"
          : "Live PostgreSQL/MySQL connections are reserved for the next phase.",
      serverVersion: connection?.driver === "sqlite" ? "mock-sqlite-3.x" : null,
    } satisfies DatabaseTestResult) as T;
  }

  if (command === "database_schema_get") {
    const connectionId = String(args?.connectionId ?? "");
    return ({
      connectionId,
      tables: [
        {
          name: "api_history",
          kind: "table",
          columns: [
            { name: "id", dataType: "TEXT", nullable: false, primaryKey: true },
            { name: "method", dataType: "TEXT", nullable: false, primaryKey: false },
            { name: "status", dataType: "INTEGER", nullable: true, primaryKey: false },
          ],
        },
        {
          name: "workspaces",
          kind: "table",
          columns: [
            { name: "id", dataType: "TEXT", nullable: false, primaryKey: true },
            { name: "name", dataType: "TEXT", nullable: false, primaryKey: false },
          ],
        },
      ],
    } satisfies DatabaseSchema) as T;
  }

  if (command === "database_query_execute") {
    const input = args?.input as DatabaseQueryInput;
    const isSelect = input.sql.trim().toLowerCase().startsWith("select");
    const keyword = input.sql.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
    const requiresConfirmation = !["select", "with", "pragma", "explain"].includes(keyword);
    if (requiresConfirmation && !input.confirmMutation) {
      throw {
        code: "CONFIRMATION_REQUIRED",
        message: "confirmation required: This SQL statement may change data. Confirm to execute it.",
        details: {
          classification: ["insert", "update", "delete", "replace"].includes(keyword)
            ? "mutation"
            : "schema-change",
          requiresConfirmation: true,
          confirmed: false,
        },
      };
    }
    return ({
      columns: isSelect
        ? [
            { name: "id", dataType: "TEXT" },
            { name: "name", dataType: "TEXT" },
            { name: "sync_status", dataType: "TEXT" },
          ]
        : [],
      rows: isSelect
        ? [
            ["mock-workspace", "Default Workspace", "local"],
            ["mock-api", "API Client", "local"],
          ]
        : [],
      affectedRows: isSelect ? 0 : 1,
      durationMs: 7,
      safety: {
        classification: isSelect ? "read" : "mutation",
        requiresConfirmation,
        confirmed: !requiresConfirmation || input.confirmMutation === true,
        message: requiresConfirmation
          ? "This SQL statement may change data. Confirm to execute it."
          : null,
      },
    } satisfies DatabaseQueryResult) as T;
  }

  if (command === "database_table_browse") {
    const input = args?.input as DatabaseBrowseInput;
    const limit = input.limit ?? 100;
    const offset = input.offset ?? 0;
    const mockRows = [
      ["mock-workspace", "Default Workspace", "local"],
      ["mock-api", "API Client", "local"],
      ["mock-db", "Database", "local"],
      ["mock-ssh", "SSH Terminal", "reserved"],
    ];
    return ({
      tableName: input.tableName,
      sql: `SELECT * FROM "${input.tableName.split('"').join('""')}" LIMIT ${limit} OFFSET ${offset}`,
      limit,
      offset,
      totalRows: mockRows.length,
      readOnly: true,
      result: {
        columns: [
          { name: "id", dataType: "TEXT" },
          { name: "name", dataType: "TEXT" },
          { name: "sync_status", dataType: "TEXT" },
        ],
        rows: mockRows.slice(offset, offset + limit),
        affectedRows: 0,
        durationMs: 5,
        safety: {
          classification: "read",
          requiresConfirmation: false,
          confirmed: true,
          message: null,
        },
      },
    } satisfies DatabaseBrowseResult) as T;
  }

  if (command === "ssh_connections_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockSshConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "ssh_connection_save") {
    const input = args?.input as SshConnectionInput;
    const now = new Date().toISOString();
    const existingIndex = input.id
      ? mockSshConnections.findIndex((item) => item.id === input.id)
      : -1;
    const connection: SshConnection = {
      id: input.id || crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name.trim(),
      host: input.host.trim(),
      port: input.port || 22,
      username: input.username.trim(),
      authKind: input.authKind,
      keyPath: input.keyPath?.trim() || null,
      credentialRef: input.credentialRef?.trim() || null,
      createdAt: existingIndex >= 0 ? mockSshConnections[existingIndex].createdAt : now,
      updatedAt: now,
      deletedAt: null,
      revision: existingIndex >= 0 ? mockSshConnections[existingIndex].revision + 1 : 1,
      syncStatus: existingIndex >= 0 ? "pending" : "local",
      remoteId: null,
    };
    if (existingIndex >= 0) {
      mockSshConnections[existingIndex] = connection;
    } else {
      mockSshConnections = [connection, ...mockSshConnections];
    }
    return connection as T;
  }

  if (command === "ssh_connection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const connectionId = String(args?.connectionId ?? "");
    mockSshConnections = mockSshConnections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === connectionId),
    );
    return mockSshConnections.filter((item) => item.workspaceId === workspaceId) as T;
  }

  throw new Error(`Mock command is not implemented: ${command}`);
}

export function getSystemHealth() {
  return call<SystemHealth>("system_health");
}

export function getWorkspaceState() {
  return call<WorkspaceState>("workspace_list");
}

export function createWorkspace(name: string) {
  return call<Workspace>("workspace_create", { name });
}

export function renameWorkspace(workspaceId: string, name: string) {
  return call<Workspace>("workspace_rename", { workspaceId, name });
}

export function deleteWorkspace(workspaceId: string) {
  return call<WorkspaceState>("workspace_delete", { workspaceId });
}

export function setActiveWorkspace(workspaceId: string) {
  return call<WorkspaceState>("workspace_set_active", { workspaceId });
}

export function getWorkspaceEnvironment(workspaceId: string) {
  return call<WorkspaceEnvironment>("workspace_environment_get", { workspaceId });
}

export function updateWorkspaceEnvironment(
  workspaceId: string,
  variables: WorkspaceEnvironment["variables"],
) {
  return call<WorkspaceEnvironment>("workspace_environment_update", {
    workspaceId,
    variables,
  });
}

export function getWorkspaceLayout(workspaceId: string) {
  return call<WorkspaceLayout>("workspace_layout_get", { workspaceId });
}

export function updateWorkspaceLayout(workspaceId: string, layout: WorkspaceLayout) {
  return call<WorkspaceLayout>("workspace_layout_update", {
    workspaceId,
    layout,
  });
}

export function sendApiRequest(input: ApiRequestInput) {
  return call<ApiResponse>("api_send_request", { input });
}

export function saveApiRequest(input: ApiRequestInput) {
  return call<ApiSavedRequest>("api_request_save", { input });
}

export function duplicateApiRequest(workspaceId: string, requestId: string) {
  return call<ApiSavedRequest>("api_request_duplicate", { workspaceId, requestId });
}

export function deleteApiRequest(workspaceId: string, requestId: string) {
  return call<ApiSavedRequest[]>("api_request_delete", { workspaceId, requestId });
}

export function listApiHistory(workspaceId: string) {
  return call<ApiHistoryItem[]>("api_history_list", { workspaceId, limit: 50 });
}

export function getApiHistoryDetail(workspaceId: string, historyId: string) {
  return call<ApiHistoryDetail>("api_history_detail", { workspaceId, historyId });
}

export function listSavedApiRequests(workspaceId: string) {
  return call<ApiSavedRequest[]>("api_saved_requests", { workspaceId });
}

export function createCredential(input: CredentialCreateInput) {
  return call<CredentialMetadata>("credential_create", { input });
}

export function deleteCredential(input: CredentialDeleteInput) {
  return call<void>("credential_delete", { input });
}

export function listDatabaseConnections(workspaceId: string) {
  return call<DatabaseConnection[]>("database_connections_list", { workspaceId });
}

export function saveDatabaseConnection(input: DatabaseConnectionInput) {
  return call<DatabaseConnection>("database_connection_save", { input });
}

export function deleteDatabaseConnection(workspaceId: string, connectionId: string) {
  return call<DatabaseConnection[]>("database_connection_delete", {
    workspaceId,
    connectionId,
  });
}

export function testDatabaseConnection(workspaceId: string, connectionId: string) {
  return call<DatabaseTestResult>("database_connection_test", {
    workspaceId,
    connectionId,
  });
}

export function getDatabaseSchema(workspaceId: string, connectionId: string) {
  return call<DatabaseSchema>("database_schema_get", {
    workspaceId,
    connectionId,
  });
}

export function executeDatabaseQuery(input: DatabaseQueryInput) {
  return call<DatabaseQueryResult>("database_query_execute", { input });
}

export function browseDatabaseTable(input: DatabaseBrowseInput) {
  return call<DatabaseBrowseResult>("database_table_browse", { input });
}

export function listSshConnections(workspaceId: string) {
  return call<SshConnection[]>("ssh_connections_list", { workspaceId });
}

export function saveSshConnection(input: SshConnectionInput) {
  return call<SshConnection>("ssh_connection_save", { input });
}

export function deleteSshConnection(workspaceId: string, connectionId: string) {
  return call<SshConnection[]>("ssh_connection_delete", {
    workspaceId,
    connectionId,
  });
}

function resolveInput(input: ApiRequestInput, variables: WorkspaceEnvironment["variables"]) {
  return {
    ...input,
    url: resolveTemplate(input.url, variables),
    headers: input.headers.map((item) => ({
      ...item,
      key: resolveTemplate(item.key, variables),
      value: resolveTemplate(item.value, variables),
    })),
    query: input.query.map((item) => ({
      ...item,
      key: resolveTemplate(item.key, variables),
      value: resolveTemplate(item.value, variables),
    })),
    body: input.body ? resolveTemplate(input.body, variables) : input.body,
  };
}

function resolveTemplate(value: string, variables: WorkspaceEnvironment["variables"]) {
  return variables
    .filter((item) => item.enabled && item.key)
    .reduce(
      (current, item) => current.split(`{{${item.key}}}`).join(item.value),
      value,
    );
}

function redactHeaders(headers: ApiRequestInput["headers"]) {
  const sensitive = new Set([
    "authorization",
    "cookie",
    "proxy-authorization",
    "x-api-key",
    "x-auth-token",
  ]);
  return headers.map((item) => ({
    ...item,
    value: sensitive.has(item.key.toLowerCase()) ? "<redacted>" : item.value,
  }));
}

function normalizeFolderPath(value: ApiRequestInput["folderPath"]) {
  const trimmed = value?.trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  return trimmed ? trimmed : null;
}

function getMockLayout(workspaceId: string): WorkspaceLayout {
  mockLayouts[workspaceId] ??= {
    workspaceId,
    sidebarCollapsed: false,
    activeTabId: "api-main",
    tabs: [
      { id: "api-main", title: "API Client", kind: "api" },
      { id: "ssh-main", title: "SSH Terminal", kind: "ssh" },
      { id: "database-main", title: "Database", kind: "database" },
    ],
    selectedApiRequestId: null,
    selectedDatabaseConnectionId: null,
    selectedSshConnectionId: null,
    updatedAt: new Date().toISOString(),
  };

  return mockLayouts[workspaceId];
}
