import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
  CredentialCreateInput,
  CredentialDeleteInput,
  CredentialInspectInput,
  CredentialMetadata,
  CredentialRotateInput,
  DatabaseBrowseInput,
  DatabaseBrowseResult,
  DatabaseConnection,
  DatabaseConnectionInput,
  DbQueryHistoryEntry,
  DatabaseQueryInput,
  DatabaseQueryResult,
  DatabaseRowMutationInput,
  DatabaseRowMutationResult,
  DatabaseSchema,
  DatabaseTableStructure,
  DatabaseTableStructureInput,
  DatabaseTestResult,
  KeyValue,
  SavedSql,
  SavedSqlInput,
  SshCloseInput,
  SshConnectInput,
  SshConnection,
  SshConnectionInput,
  SshHostFingerprintInfo,
  SshHostKeyInput,
  SshKnownHostsExportResult,
  SshKnownHostsImportInput,
  SshKnownHostsImportResult,
  SshLogExport,
  SshLogExportInput,
  SshResizeInput,
  SshReconnectCancelInput,
  SshSessionEvent,
  SshSessionInput,
  SshSessionSummary,
  SystemHealth,
  ApiCollection,
  ApiEnvironment,
  Workspace,
  WorkspaceLayout,
  WorkspaceState,
} from "./types";

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
let mockSavedSql: SavedSql[] = [];
let mockSshConnections: SshConnection[] = [];
let mockSshSessions: SshSessionSummary[] = [];
const mockSshEvents: SshSessionEvent[] = [];
const MOCK_TERMINAL_HISTORY_MAX_BYTES = 256 * 1024;
const mockHostKeyFingerprints: Record<string, SshHostFingerprintInfo> = {};
const mockCredentials: Record<string, string> = {};
let mockEnvironments: ApiEnvironment[] = [
  {
    id: "env-default-mock",
    workspaceId: mockWorkspace.id,
    name: "Default",
    variables: [
      { key: "base_url", value: "https://httpbin.org", enabled: true },
      { key: "source", value: "unfour", enabled: true },
    ],
    isActive: true,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

let mockCollections: ApiCollection[] = [];

function mockCollectionList(workspaceId: string) {
  return mockCollections
    .filter((collection) => collection.workspaceId === workspaceId)
    .sort((a, b) => a.name.localeCompare(b.name));
}

function assertMockCollection(workspaceId: string, collectionId: string | null | undefined) {
  if (
    collectionId &&
    !mockCollections.some(
      (collection) =>
        collection.workspaceId === workspaceId && collection.id === collectionId,
    )
  ) {
    throw new Error("api collection not found");
  }
}

function mockEnvList(workspaceId: string) {
  return mockEnvironments
    .filter((env) => env.workspaceId === workspaceId)
    .sort((a, b) => a.name.localeCompare(b.name));
}

function normalizeMockEnvironmentName(name: string) {
  return name.trim().toLowerCase();
}

function assertMockEnvironmentNameAvailable(
  workspaceId: string,
  name: string,
  excludeId?: string,
) {
  const normalized = normalizeMockEnvironmentName(name);
  if (
    normalized &&
    mockEnvironments.some(
      (env) =>
        env.workspaceId === workspaceId &&
        env.id !== excludeId &&
        normalizeMockEnvironmentName(env.name) === normalized,
    )
  ) {
    throw new Error(`environment name already exists in this workspace: ${name}`);
  }
}

function mockActiveEnvVariables(workspaceId: string): KeyValue[] {
  return (
    mockEnvironments.find((env) => env.workspaceId === workspaceId && env.isActive)
      ?.variables ?? []
  );
}
const mockLayouts: Record<string, WorkspaceLayout> = {};

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
      appName: "Unfour",
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

  if (command === "api_environments_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockEnvList(workspaceId) as T;
  }

  if (command === "api_environment_create") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const name = String(args?.name ?? "New Environment").trim() || "New Environment";
    assertMockEnvironmentNameAvailable(workspaceId, name);
    const isActive = mockEnvList(workspaceId).length === 0;
    const now = new Date().toISOString();
    const environment: ApiEnvironment = {
      id: crypto.randomUUID(),
      workspaceId,
      name,
      variables: [],
      isActive,
      createdAt: now,
      updatedAt: now,
    };
    mockEnvironments = [...mockEnvironments, environment];
    return environment as T;
  }

  if (command === "api_environment_update") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const environmentId = String(args?.environmentId ?? "");
    const environment = mockEnvironments.find(
      (env) => env.workspaceId === workspaceId && env.id === environmentId,
    );
    if (!environment) throw new Error("api environment not found");
    const name = String(args?.name ?? environment.name).trim() || environment.name;
    assertMockEnvironmentNameAvailable(workspaceId, name, environmentId);
    environment.name = name;
    environment.variables = (args?.variables as KeyValue[]) ?? [];
    environment.updatedAt = new Date().toISOString();
    return environment as T;
  }

  if (command === "api_environment_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const environmentId = String(args?.environmentId ?? "");
    mockEnvironments = mockEnvironments.filter(
      (env) => !(env.workspaceId === workspaceId && env.id === environmentId),
    );
    return mockEnvList(workspaceId) as T;
  }

  if (command === "api_environment_activate") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const environmentId = args?.environmentId ? String(args.environmentId) : null;
    mockEnvironments = mockEnvironments.map((env) =>
      env.workspaceId === workspaceId
        ? { ...env, isActive: env.id === environmentId }
        : env,
    );
    return mockEnvList(workspaceId) as T;
  }

  if (command === "api_collection_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockCollectionList(workspaceId) as T;
  }

  if (command === "api_collection_create") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const now = new Date().toISOString();
    const collection: ApiCollection = {
      id: crypto.randomUUID(),
      workspaceId,
      name: String(args?.name ?? "New Collection").trim() || "New Collection",
      description: null,
      folders: [],
      createdAt: now,
      updatedAt: now,
    };
    mockCollections = [...mockCollections, collection];
    return collection as T;
  }

  if (command === "api_collection_rename") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const collection = mockCollections.find(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!collection) throw new Error("api collection not found");
    collection.name = String(args?.name ?? collection.name).trim() || collection.name;
    collection.updatedAt = new Date().toISOString();
    return collection as T;
  }

  if (command === "api_collection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const existed = mockCollections.some(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!existed) throw new Error("api collection not found");
    mockCollections = mockCollections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === collectionId),
    );
    // Cascade: drop the collection's saved requests.
    mockSavedRequests = mockSavedRequests.filter(
      (item) => !(item.workspaceId === workspaceId && item.collectionId === collectionId),
    );
    return mockCollectionList(workspaceId) as T;
  }

  if (command === "api_collection_add_folder") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const collectionId = String(args?.collectionId ?? "");
    const folderPath = normalizeFolderPath(
      (args?.folderPath as string | null | undefined) ?? null,
    );
    const collection = mockCollections.find(
      (item) => item.workspaceId === workspaceId && item.id === collectionId,
    );
    if (!collection) throw new Error("api collection not found");
    if (folderPath && !collection.folders.includes(folderPath)) {
      collection.folders = [...collection.folders, folderPath].sort();
    }
    collection.updatedAt = new Date().toISOString();
    return collection as T;
  }

  if (command === "api_request_move") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const requestId = String(args?.requestId ?? "");
    const collectionId = args?.collectionId ? String(args.collectionId) : null;
    const folderPath = normalizeFolderPath(
      (args?.folderPath as string | null | undefined) ?? null,
    );
    const request = mockSavedRequests.find(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (!request) throw new Error("api request not found");
    if (
      collectionId &&
      !mockCollections.some(
        (item) => item.workspaceId === workspaceId && item.id === collectionId,
      )
    ) {
      throw new Error("api collection not found");
    }
    request.collectionId = collectionId;
    request.folderPath = folderPath;
    request.updatedAt = new Date().toISOString();
    return request as T;
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
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockSavedRequests.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "api_request_save") {
    const input = args?.input as ApiRequestInput;
    assertMockCollection(input.workspaceId, input.collectionId);
    const saved: ApiSavedRequest = {
      id: crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name || `${input.method} ${input.url}`,
      folderPath: normalizeFolderPath(input.folderPath),
      collectionId: input.collectionId ?? null,
      authJson: input.authJson ?? JSON.stringify({ type: "none" }),
      method: input.method,
      url: input.url,
      headersJson: JSON.stringify(redactHeaders(input.headers)),
      queryJson: JSON.stringify(input.query),
      body: redactJsonBody(input.body),
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

  if (command === "api_request_update") {
    const input = args?.input as ApiRequestInput;
    const workspaceId = String(args?.workspaceId ?? input.workspaceId);
    const requestId = String(args?.requestId ?? "");
    if (workspaceId !== input.workspaceId) throw new Error("api request workspace mismatch");
    assertMockCollection(workspaceId, input.collectionId);
    const index = mockSavedRequests.findIndex(
      (item) => item.workspaceId === workspaceId && item.id === requestId,
    );
    if (index === -1) throw new Error("api request not found");
    const current = mockSavedRequests[index];
    const saved: ApiSavedRequest = {
      ...current,
      name: input.name || `${input.method} ${input.url}`,
      folderPath: normalizeFolderPath(input.folderPath),
      collectionId: input.collectionId ?? null,
      authJson: input.authJson ?? JSON.stringify({ type: "none" }),
      method: input.method,
      url: input.url,
      headersJson: JSON.stringify(redactHeaders(input.headers)),
      queryJson: JSON.stringify(input.query),
      body: redactJsonBody(input.body),
      bodyKind: input.bodyKind,
      updatedAt: new Date().toISOString(),
      revision: current.revision + 1,
      syncStatus: "pending",
    };
    mockSavedRequests = [
      ...mockSavedRequests.slice(0, index),
      saved,
      ...mockSavedRequests.slice(index + 1),
    ];
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
    const resolved = resolveInput(input, mockActiveEnvVariables(input.workspaceId));
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
        requestBody: redactJsonBody(input.body),
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
    const credentialRef = `unfour:${input.workspaceId}:${input.kind}:${crypto.randomUUID()}`;
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

  if (command === "credential_inspect") {
    const input = args?.input as CredentialInspectInput;
    const metadata = inspectMockCredential(input.workspaceId, input.credentialRef);
    return metadata as T;
  }

  if (command === "credential_rotate") {
    const input = args?.input as CredentialRotateInput;
    const metadata = inspectMockCredential(input.workspaceId, input.credentialRef);
    mockCredentials[input.credentialRef] = input.secret;
    return {
      ...metadata,
      label: "Rotated credential",
    } as T;
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
      readOnly: input.readOnly ?? false,
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
    const driver = connection?.driver ?? "sqlite";
    const isSqlite = driver === "sqlite";
    const isPostgres = driver === "postgres";
    const isMySql = driver === "mysql";
    const ok = isSqlite || isPostgres || isMySql;
    return ({
      ok,
      message: `${isSqlite ? "SQLite" : isPostgres ? "PostgreSQL" : "MySQL"} connection OK`,
      serverVersion: isSqlite
        ? "mock-sqlite-3.x"
        : isPostgres
          ? "mock-postgresql-16.x"
          : "mock-mysql-8.x",
    } satisfies DatabaseTestResult) as T;
  }

  if (command === "database_catalogs_list") {
    const connectionId = String(args?.connectionId ?? "");
    const connection = mockDatabaseConnections.find((item) => item.id === connectionId);
    if (connection?.driver === "mysql") {
      return ["app", "analytics"] as T;
    }
    if (connection?.driver === "postgres") {
      // Server-level database list; the connection's default plus a second db
      // demonstrate browsing beyond the connected database.
      return [connection.database ?? "postgres", "reporting"] as T;
    }
    return [] as T;
  }

  if (command === "database_schema_get") {
    const connectionId = String(args?.connectionId ?? "");
    const catalogArg = args?.catalog ? String(args.catalog) : null;
    const connection = mockDatabaseConnections.find((item) => item.id === connectionId);
    const isPostgres = connection?.driver === "postgres";
    const isMySql = connection?.driver === "mysql";
    if (isMySql) {
      return ({
        connectionId,
        tables: [
          {
            catalog: connection.database ?? "app",
            name: "users",
            kind: "table",
            columns: [
              { name: "id", dataType: "bigint unsigned", nullable: false, primaryKey: true },
              { name: "email", dataType: "varchar(255)", nullable: false, primaryKey: false },
              { name: "created_at", dataType: "datetime", nullable: false, primaryKey: false },
            ],
          },
          {
            catalog: "analytics",
            name: "events",
            kind: "table",
            columns: [
              { name: "id", dataType: "bigint unsigned", nullable: false, primaryKey: true },
              { name: "event_name", dataType: "varchar(255)", nullable: false, primaryKey: false },
            ],
          },
        ],
      } satisfies DatabaseSchema) as T;
    }
    if (isPostgres) {
      const pgCatalog = catalogArg ?? connection.database ?? "postgres";
      // A second database (reporting) shows distinct objects, demonstrating
      // catalog-scoped schema browsing on a single connection.
      if (pgCatalog === "reporting") {
        return ({
          connectionId,
          tables: [
            {
              catalog: pgCatalog,
              schema: "metrics",
              name: "daily_revenue",
              kind: "view",
              columns: [
                { name: "day", dataType: "date", nullable: false, primaryKey: false },
                { name: "amount", dataType: "numeric", nullable: true, primaryKey: false },
              ],
            },
          ],
        } satisfies DatabaseSchema) as T;
      }
      return ({
        connectionId,
        tables: [
          {
            catalog: pgCatalog,
            schema: "public",
            name: "users",
            kind: "table",
            columns: [
              { name: "id", dataType: "integer", nullable: false, primaryKey: true },
              { name: "email", dataType: "character varying", nullable: false, primaryKey: false },
              { name: "created_at", dataType: "timestamp with time zone", nullable: false, primaryKey: false },
            ],
          },
          {
            catalog: pgCatalog,
            schema: "public",
            name: "orders",
            kind: "table",
            columns: [
              { name: "id", dataType: "integer", nullable: false, primaryKey: true },
              { name: "user_id", dataType: "integer", nullable: false, primaryKey: false },
              { name: "total", dataType: "numeric", nullable: true, primaryKey: false },
            ],
          },
        ],
      } satisfies DatabaseSchema) as T;
    }
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

  if (command === "database_query_history_list") {
    return [] as T;
  }

  if (command === "database_query_history_record") {
    return undefined as T;
  }

  if (command === "database_query_history_clear") {
    return undefined as T;
  }

  if (command === "database_saved_sql_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockSavedSql.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_saved_sql_save") {
    const input = args?.input as SavedSqlInput;
    const workspaceId = input.workspaceId;
    const id = input.id?.trim();
    const name = input.name.trim();
    const sql = input.sql.trim();
    if (!name) throw new Error("saved SQL name cannot be empty");
    if (name.length > 120) throw new Error("saved SQL name must be 120 characters or fewer");
    if (!sql) throw new Error("saved SQL cannot be empty");
    const now = new Date().toISOString();
    const existingIndex = id
      ? mockSavedSql.findIndex((item) => item.id === id && item.workspaceId === workspaceId)
      : -1;
    if (id && existingIndex === -1) throw new Error("saved SQL not found");
    const saved: SavedSql = {
      id: id || crypto.randomUUID(),
      workspaceId,
      connectionId: input.connectionId ?? null,
      name,
      sql,
      createdAt: existingIndex >= 0 ? mockSavedSql[existingIndex].createdAt : now,
      updatedAt: now,
    };
    if (existingIndex >= 0) {
      mockSavedSql = [
        saved,
        ...mockSavedSql.filter((_item, index) => index !== existingIndex),
      ];
    } else {
      mockSavedSql = [saved, ...mockSavedSql];
    }
    return saved as T;
  }

  if (command === "database_saved_sql_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const id = String(args?.id ?? "");
    const initialLength = mockSavedSql.length;
    mockSavedSql = mockSavedSql.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === id),
    );
    if (mockSavedSql.length === initialLength) throw new Error("saved SQL not found");
    return mockSavedSql.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_query_execute") {
    const input = args?.input as DatabaseQueryInput;
    const isSelect = input.sql.trim().toLowerCase().startsWith("select");
    const keyword = input.sql.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
    const requiresConfirmation = !["select", "with", "pragma", "explain", "show"].includes(keyword);
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
    const connection = mockDatabaseConnections.find((item) => item.id === input.connectionId);
    const qualifiedTable =
      connection?.driver === "mysql"
        ? `${quoteMySqlIdentifier(input.catalog ?? input.schema ?? connection.database ?? "app")}.${quoteMySqlIdentifier(input.tableName)}`
        : `"${input.tableName.split('"').join('""')}"`;
    return ({
      tableName: input.tableName,
      sql: `SELECT * FROM ${qualifiedTable} LIMIT ${limit} OFFSET ${offset}`,
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

  if (command === "database_table_structure") {
    const input = args?.input as DatabaseTableStructureInput;
    return ({
      catalog: input.catalog ?? null,
      schema: input.schema ?? null,
      name: input.tableName,
      kind: "table",
      columns: [
        { name: "id", dataType: "TEXT", nullable: false, primaryKey: true, defaultValue: null },
        { name: "name", dataType: "TEXT", nullable: false, primaryKey: false, defaultValue: null },
        {
          name: "sync_status",
          dataType: "TEXT",
          nullable: true,
          primaryKey: false,
          defaultValue: "'local'",
        },
      ],
      indexes: [{ name: "PRIMARY", columns: ["id"], unique: true, primary: true }],
      foreignKeys: [],
      ddl: `CREATE TABLE ${input.tableName} (\n  id TEXT PRIMARY KEY,\n  name TEXT NOT NULL,\n  sync_status TEXT DEFAULT 'local'\n);`,
    } satisfies DatabaseTableStructure) as T;
  }

  if (command === "database_row_mutate") {
    const input = args?.input as DatabaseRowMutationInput;
    return ({
      affectedRows: 1,
      sql: `-- mock ${input.operation} on ${input.tableName}`,
    } satisfies DatabaseRowMutationResult) as T;
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

  if (command === "ssh_session_connect") {
    const input = args?.input as SshConnectInput;
    const connection = mockSshConnections.find(
      (item) => item.workspaceId === input.workspaceId && item.id === input.connectionId,
    );
    if (!connection) throw new Error("ssh connection not found");
    const now = new Date().toISOString();
    const session: SshSessionSummary = {
      sessionId: crypto.randomUUID(),
      workspaceId: input.workspaceId,
      connectionId: input.connectionId,
      status: "connected",
      reconnectAttempt: 0,
      authKind: connection.authKind,
      host: connection.host,
      username: connection.username,
      cols: input.cols ?? 120,
      rows: input.rows ?? 32,
      createdAt: now,
      updatedAt: now,
    };
    mockSshSessions = [session, ...mockSshSessions];
    mockSshEvents.push({
      sessionId: session.sessionId,
      kind: "output",
      data: `Connected to ${session.username}@${session.host} with ${session.authKind} auth. PTY ${session.cols}x${session.rows} allocated.\r\n`,
      createdAt: now,
    });
    trimMockSshHistory(session.sessionId);
    // Simulate TOFU: record a mock fingerprint if not already stored.
    const hostKey = `${connection.host}:${connection.port}`;
    if (!(hostKey in mockHostKeyFingerprints)) {
      mockHostKeyFingerprints[hostKey] = {
        host: connection.host,
        port: connection.port,
        fingerprint: `SHA256:mock-${crypto.randomUUID().slice(0, 12)}`,
        createdAt: now,
      };
    }
    return session as T;
  }

  if (command === "ssh_sessions_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockSshSessions.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "ssh_session_input") {
    const input = args?.input as SshSessionInput;
    const session = mockSshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    if (session.status !== "connected") throw new Error("ssh session is not connected");
    const now = new Date().toISOString();
    mockSshEvents.push({
      sessionId: input.sessionId,
      kind: "input",
      data: redactSshLog(input.data),
      createdAt: now,
    });
    const event: SshSessionEvent = {
      sessionId: input.sessionId,
      kind: "output",
      data: "Input accepted by SSH PTY stream.\r\n",
      createdAt: now,
    };
    mockSshEvents.push(event);
    trimMockSshHistory(input.sessionId);
    session.updatedAt = now;
    return event as T;
  }

  if (command === "ssh_session_resize") {
    const input = args?.input as SshResizeInput;
    const session = mockSshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.cols = input.cols;
    session.rows = input.rows;
    session.updatedAt = now;
    const event: SshSessionEvent = {
      sessionId: input.sessionId,
      kind: "resize",
      data: `PTY resized to ${input.cols}x${input.rows}.\r\n`,
      createdAt: now,
    };
    mockSshEvents.push(event);
    return event as T;
  }

  if (command === "ssh_session_close") {
    const input = args?.input as SshCloseInput;
    const session = mockSshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.status = "disconnected";
    session.reconnectAttempt = 0;
    session.updatedAt = now;
    mockSshEvents.push({
      sessionId: input.sessionId,
      kind: "close",
      data: "SSH session closed.\r\n",
      createdAt: now,
    });
    return session as T;
  }

  if (command === "ssh_session_history") {
    const input = args?.input as SshCloseInput;
    const session = mockSshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) return [] as T;
    return mockSshEvents
      .filter((event) => event.sessionId === input.sessionId && event.kind !== "input")
      .map((event) => ({ ...event, data: redactSshLog(event.data) })) as T;
  }

  if (command === "ssh_session_reconnect_cancel") {
    const input = args?.input as SshReconnectCancelInput;
    const session = mockSshSessions.find(
      (item) => item.workspaceId === input.workspaceId && item.sessionId === input.sessionId,
    );
    if (!session) throw new Error("ssh session not found");
    const now = new Date().toISOString();
    session.status = "disconnected";
    session.reconnectAttempt = 0;
    session.updatedAt = now;
    mockSshEvents.push({
      sessionId: input.sessionId,
      kind: "close",
      data: "SSH reconnect cancelled.\r\n",
      createdAt: now,
    });
    return session as T;
  }

  if (command === "ssh_session_log_export") {
    const input = args?.input as SshLogExportInput;
    const events = mockSshEvents.filter((item) => item.sessionId === input.sessionId);
    const content = events
      .map((event) => `[${event.createdAt}] ${event.kind} ${redactSshLog(event.data)}`)
      .join("\n");
    return ({
      sessionId: input.sessionId,
      filename: `ssh-session-${input.sessionId}.log`,
      content,
      lineCount: events.length,
      redacted: content.includes("<redacted>"),
    } satisfies SshLogExport) as T;
  }

  if (command === "ssh_host_key_get") {
    const input = args?.input as SshHostKeyInput;
    const key = `${input.host}:${input.port}`;
    const info = mockHostKeyFingerprints[key];
    return (info ?? null) as T;
  }

  if (command === "ssh_host_key_reset") {
    const input = args?.input as SshHostKeyInput;
    const key = `${input.host}:${input.port}`;
    const existed = key in mockHostKeyFingerprints;
    delete mockHostKeyFingerprints[key];
    return existed as T;
  }

  if (command === "ssh_host_key_list") {
    return Object.values(mockHostKeyFingerprints) as T;
  }

  if (command === "ssh_known_hosts_import") {
    const input = args?.input as SshKnownHostsImportInput;
    const lines = input.content.split("\n");
    let imported = 0;
    let skipped = 0;
    const errors: string[] = [];
    const now = new Date().toISOString();
    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#")) continue;
      const parts = line.split(/\s+/);
      if (parts.length < 3 || (!parts[1].startsWith("ssh-") && !parts[1].startsWith("ecdsa-"))) {
        skipped++;
        continue;
      }
      const hostField = parts[0];
      let host: string;
      let port = 22;
      if (hostField.startsWith("[")) {
        const bracketEnd = hostField.indexOf("]");
        if (bracketEnd < 0) { skipped++; continue; }
        host = hostField.slice(1, bracketEnd);
        const rest = hostField.slice(bracketEnd + 1);
        if (rest.startsWith(":")) port = parseInt(rest.slice(1), 10) || 22;
      } else {
        host = hostField;
      }
      const key = `${host}:${port}`;
      if (key in mockHostKeyFingerprints) {
        skipped++;
        continue;
      }
      mockHostKeyFingerprints[key] = {
        host,
        port,
        fingerprint: `SHA256:mock-${crypto.randomUUID().slice(0, 12)}`,
        createdAt: now,
      };
      imported++;
    }
    return { imported, skipped, errors } as T;
  }

  if (command === "ssh_known_hosts_export") {
    const entries = Object.values(mockHostKeyFingerprints);
    const lines = entries.map((e) => {
      const hostPort = e.port === 22 ? e.host : `[${e.host}]:${e.port}`;
      return `# ${hostPort} ${e.fingerprint} (fingerprint only, no key data)`;
    });
    return {
      content: lines.length > 0 ? lines.join("\n") + "\n" : "",
      entryCount: 0,
    } as T;
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

export function listApiEnvironments(workspaceId: string) {
  return call<ApiEnvironment[]>("api_environments_list", { workspaceId });
}

export function createApiEnvironment(workspaceId: string, name: string) {
  return call<ApiEnvironment>("api_environment_create", { workspaceId, name });
}

export function updateApiEnvironment(
  workspaceId: string,
  environmentId: string,
  name: string,
  variables: KeyValue[],
) {
  return call<ApiEnvironment>("api_environment_update", {
    workspaceId,
    environmentId,
    name,
    variables,
  });
}

export function deleteApiEnvironment(workspaceId: string, environmentId: string) {
  return call<ApiEnvironment[]>("api_environment_delete", {
    workspaceId,
    environmentId,
  });
}

export function activateApiEnvironment(
  workspaceId: string,
  environmentId: string | null,
) {
  return call<ApiEnvironment[]>("api_environment_activate", {
    workspaceId,
    environmentId,
  });
}

export function listApiCollections(workspaceId: string) {
  return call<ApiCollection[]>("api_collection_list", { workspaceId });
}

export function createApiCollection(workspaceId: string, name: string) {
  return call<ApiCollection>("api_collection_create", { workspaceId, name });
}

export function renameApiCollection(
  workspaceId: string,
  collectionId: string,
  name: string,
) {
  return call<ApiCollection>("api_collection_rename", {
    workspaceId,
    collectionId,
    name,
  });
}

export function deleteApiCollection(workspaceId: string, collectionId: string) {
  return call<ApiCollection[]>("api_collection_delete", {
    workspaceId,
    collectionId,
  });
}

export function addApiCollectionFolder(
  workspaceId: string,
  collectionId: string,
  folderPath: string,
) {
  return call<ApiCollection>("api_collection_add_folder", {
    workspaceId,
    collectionId,
    folderPath,
  });
}

export function moveApiRequest(
  workspaceId: string,
  requestId: string,
  collectionId: string | null,
  folderPath: string | null,
) {
  return call<ApiSavedRequest>("api_request_move", {
    workspaceId,
    requestId,
    collectionId,
    folderPath,
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

export function updateApiRequest(
  workspaceId: string,
  requestId: string,
  input: ApiRequestInput,
) {
  return call<ApiSavedRequest>("api_request_update", {
    workspaceId,
    requestId,
    input,
  });
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

export function inspectCredential(input: CredentialInspectInput) {
  return call<CredentialMetadata>("credential_inspect", { input });
}

export function rotateCredential(input: CredentialRotateInput) {
  return call<CredentialMetadata>("credential_rotate", { input });
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

export function getDatabaseSchema(
  workspaceId: string,
  connectionId: string,
  catalog?: string | null,
) {
  return call<DatabaseSchema>("database_schema_get", {
    workspaceId,
    connectionId,
    catalog: catalog ?? null,
  });
}

export function listDatabaseCatalogs(workspaceId: string, connectionId: string) {
  return call<string[]>("database_catalogs_list", {
    workspaceId,
    connectionId,
  });
}

export function executeDatabaseQuery(input: DatabaseQueryInput) {
  return call<DatabaseQueryResult>("database_query_execute", { input });
}

export function recordDatabaseQueryHistory(input: DbQueryHistoryEntry) {
  return call<void>("database_query_history_record", { input });
}

export function listDatabaseQueryHistory(workspaceId: string, limit = 200) {
  return call<DbQueryHistoryEntry[]>("database_query_history_list", { workspaceId, limit });
}

export function clearDatabaseQueryHistory(workspaceId: string) {
  return call<void>("database_query_history_clear", { workspaceId });
}

export function listSavedSql(workspaceId: string) {
  return call<SavedSql[]>("database_saved_sql_list", { workspaceId });
}

export function saveSavedSql(input: SavedSqlInput) {
  return call<SavedSql>("database_saved_sql_save", { input });
}

export function deleteSavedSql(workspaceId: string, id: string) {
  return call<SavedSql[]>("database_saved_sql_delete", { workspaceId, id });
}

export function browseDatabaseTable(input: DatabaseBrowseInput) {
  return call<DatabaseBrowseResult>("database_table_browse", { input });
}

export function getDatabaseTableStructure(input: DatabaseTableStructureInput) {
  return call<DatabaseTableStructure>("database_table_structure", { input });
}

export function mutateDatabaseRow(input: DatabaseRowMutationInput) {
  return call<DatabaseRowMutationResult>("database_row_mutate", { input });
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

export function connectSshSession(input: SshConnectInput) {
  return call<SshSessionSummary>("ssh_session_connect", { input });
}

export function listSshSessions(workspaceId: string) {
  return call<SshSessionSummary[]>("ssh_sessions_list", { workspaceId });
}

export function getSshSessionHistory(input: SshCloseInput) {
  return call<SshSessionEvent[]>("ssh_session_history", { input });
}

export function sendSshInput(input: SshSessionInput) {
  return call<SshSessionEvent>("ssh_session_input", { input });
}

export function resizeSshSession(input: SshResizeInput) {
  return call<SshSessionEvent>("ssh_session_resize", { input });
}

export function closeSshSession(input: SshCloseInput) {
  return call<SshSessionSummary>("ssh_session_close", { input });
}

export function cancelSshReconnect(input: SshReconnectCancelInput) {
  return call<SshSessionSummary>("ssh_session_reconnect_cancel", { input });
}

export function exportSshLog(input: SshLogExportInput) {
  return call<SshLogExport>("ssh_session_log_export", { input });
}

export function getSshHostFingerprint(input: SshHostKeyInput) {
  return call<SshHostFingerprintInfo | null>("ssh_host_key_get", { input });
}

export function resetSshHostFingerprint(input: SshHostKeyInput) {
  return call<boolean>("ssh_host_key_reset", { input });
}

export function listSshHostFingerprints() {
  return call<SshHostFingerprintInfo[]>("ssh_host_key_list");
}

export function importSshKnownHosts(input: SshKnownHostsImportInput) {
  return call<SshKnownHostsImportResult>("ssh_known_hosts_import", { input });
}

export function exportSshKnownHosts() {
  return call<SshKnownHostsExportResult>("ssh_known_hosts_export");
}

export type SshTerminalDataPayload = {
  sessionId: string;
  data: string;
  status?: SshSessionSummary["status"] | null;
  reconnectAttempt?: number;
};

/**
 * Subscribe to live SSH terminal output over a Tauri IPC `Channel` (the same
 * reliable transport used by commands) rather than the event system. High-rate
 * Tauri events stall WebView2 event delivery on Windows under a full-screen
 * redraw burst (vim/less/top), which silently freezes the terminal; channels do
 * not have that failure mode. Returns a disposer that detaches the handler. A
 * no-op outside the Tauri runtime (browser mock mode polls history instead).
 */
export function registerSshTerminalChannel(
  onMessage: (payload: SshTerminalDataPayload) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return Promise.resolve(() => {});
  }
  const channel = new Channel<SshTerminalDataPayload>();
  channel.onmessage = onMessage;
  return invoke<void>("ssh_register_terminal_channel", { channel })
    .then(() => () => {
      channel.onmessage = () => {};
    })
    .catch(() => () => {
      channel.onmessage = () => {};
    });
}

function resolveInput(input: ApiRequestInput, variables: KeyValue[]) {
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

function trimMockSshHistory(sessionId: string) {
  const sessionEvents = mockSshEvents.filter(
    (event) => event.sessionId === sessionId && event.kind !== "input",
  );
  let totalBytes = sessionEvents.reduce(
    (total, event) => total + new TextEncoder().encode(event.data).byteLength,
    0,
  );
  while (totalBytes > MOCK_TERMINAL_HISTORY_MAX_BYTES && sessionEvents.length > 1) {
    const removed = sessionEvents.shift();
    if (!removed) break;
    totalBytes -= new TextEncoder().encode(removed.data).byteLength;
    const index = mockSshEvents.indexOf(removed);
    if (index >= 0) mockSshEvents.splice(index, 1);
  }
}

function resolveTemplate(value: string, variables: KeyValue[]) {
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

function redactJsonBody(body: string | null | undefined): string | null {
  if (!body) return null;
  try {
    const parsed = JSON.parse(body);
    const sensitive = new Set([
      "authorization",
      "cookie",
      "proxy-authorization",
      "x-api-key",
      "x-auth-token",
    ]);
    let changed = false;
    function walk(value: unknown): unknown {
      if (value && typeof value === "object" && !Array.isArray(value)) {
        const obj = value as Record<string, unknown>;
        const result: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(obj)) {
          if (sensitive.has(k.toLowerCase())) {
            result[k] = "<redacted>";
            changed = true;
          } else {
            result[k] = walk(v);
          }
        }
        return result;
      }
      if (Array.isArray(value)) {
        return value.map(walk);
      }
      return value;
    }
    const redacted = walk(parsed);
    return changed ? JSON.stringify(redacted) : body;
  } catch {
    return body;
  }
}

function normalizeFolderPath(value: ApiRequestInput["folderPath"]) {
  const trimmed = value?.trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  return trimmed ? trimmed : null;
}

function redactSshLog(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => {
      const lower = line.toLowerCase();
      return [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "x-auth-token",
        "password",
        "passphrase",
      ].some((needle) => lower.includes(needle))
        ? "<redacted>"
        : line;
    })
    .join("\n");
}

function inspectMockCredential(
  workspaceId: string,
  credentialRef: string,
): CredentialMetadata {
  const [serviceName, refWorkspaceId, kind, recordId] = credentialRef.split(":");
  if (
    serviceName !== "unfour" ||
    refWorkspaceId !== workspaceId ||
    !kind ||
    !recordId ||
    !(credentialRef in mockCredentials)
  ) {
    throw new Error("credential not found");
  }

  return {
    workspaceId,
    kind,
    label: "Credential reference",
    credentialRef,
  };
}

function quoteMySqlIdentifier(value: string) {
  return `\`${value.split("`").join("``")}\``;
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
