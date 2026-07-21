import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiEnvironment,
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiSavedRequest,
  DatabaseConnection,
  KeyValue,
  SavedSql,
  SshConnection,
  SshHostFingerprintInfo,
  SshSessionEvent,
  SshSessionSummary,
  SshTaskDetail,
  SshTaskRun,
  Workspace,
  WorkspaceLayout,
  WorkspaceState,
} from "../../types";

export const mockWorkspace: Workspace = {
  id: "mock-workspace",
  name: "Default Workspace",
  isDefault: true,
  lastOpenedAt: new Date().toISOString(),
  environmentType: "dev",
  mcpPolicy: "auto",
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  deletedAt: null,
  revision: 1,
  syncStatus: "local",
  remoteId: null,
};

export const mockState: WorkspaceState = {
  activeWorkspaceId: mockWorkspace.id,
  workspaces: [mockWorkspace],
};

export const mockStore = {
  history: [] as ApiHistoryItem[],
  historyDetails: [] as ApiHistoryDetail[],
  savedRequests: [] as ApiSavedRequest[],
  databaseConnections: [] as DatabaseConnection[],
  savedSql: [] as SavedSql[],
  sshConnections: [] as SshConnection[],
  sshSessions: [] as SshSessionSummary[],
  sshEvents: [] as SshSessionEvent[],
  sshTasks: [] as SshTaskDetail[],
  sshTaskRuns: [] as SshTaskRun[],
  hostKeyFingerprints: {} as Record<string, SshHostFingerprintInfo>,
  credentials: {} as Record<string, string>,
  environments: [
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
  ] as ApiEnvironment[],
  collections: [] as ApiCollection[],
  collectionFolders: [] as ApiCollectionFolder[],
  layouts: {} as Record<string, WorkspaceLayout>,
};

const MOCK_TERMINAL_HISTORY_MAX_BYTES = 256 * 1024;

export function mockHostKeyFingerprintKey(
  workspaceId: string,
  host: string,
  port: number,
) {
  return `${workspaceId}:${host}:${port}`;
}

export function mockCollectionList(workspaceId: string) {
  return mockStore.collections
    .filter((collection) => collection.workspaceId === workspaceId)
    .sort((a, b) => a.name.localeCompare(b.name));
}

export function firstOrCreateMockCollectionId(workspaceId: string) {
  const first = mockCollectionList(workspaceId)[0];
  if (first) return first.id;
  const now = new Date().toISOString();
  const collection: ApiCollection = {
    id: crypto.randomUUID(),
    workspaceId,
    name: "Default Collection",
    description: null,
    createdAt: now,
    updatedAt: now,
  };
  mockStore.collections = [...mockStore.collections, collection];
  return collection.id;
}

export function assertMockCollection(
  workspaceId: string,
  collectionId: string | null | undefined,
) {
  if (
    collectionId &&
    !mockStore.collections.some(
      (collection) =>
        collection.workspaceId === workspaceId && collection.id === collectionId,
    )
  ) {
    throw new Error("api collection not found");
  }
}

export function normalizeMockId(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function assertMockFolder(
  workspaceId: string,
  collectionId: string,
  folderId: string | null | undefined,
) {
  if (
    folderId &&
    !mockStore.collectionFolders.some(
      (folder) =>
        folder.workspaceId === workspaceId &&
        folder.collectionId === collectionId &&
        folder.id === folderId,
    )
  ) {
    throw new Error("api collection folder not found");
  }
}

export function mockFolderList(workspaceId: string, collectionId?: string | null) {
  return mockStore.collectionFolders
    .filter(
      (folder) =>
        folder.workspaceId === workspaceId &&
        (!collectionId || folder.collectionId === collectionId),
    )
    .sort((left, right) => {
      const collectionSort = left.collectionId.localeCompare(right.collectionId);
      if (collectionSort !== 0) return collectionSort;
      const parentSort = (left.parentFolderId ?? "").localeCompare(
        right.parentFolderId ?? "",
      );
      if (parentSort !== 0) return parentSort;
      return left.sortOrder - right.sortOrder || left.name.localeCompare(right.name);
    });
}

export function nextMockFolderSortOrder(
  workspaceId: string,
  collectionId: string,
  parentFolderId: string | null,
) {
  return (
    Math.max(
      -1,
      ...mockStore.collectionFolders
        .filter(
          (folder) =>
            folder.workspaceId === workspaceId &&
            folder.collectionId === collectionId &&
            folder.parentFolderId === parentFolderId,
        )
        .map((folder) => folder.sortOrder),
    ) + 1
  );
}

export function nextMockRequestSortOrder(
  workspaceId: string,
  collectionId: string,
  parentFolderId: string | null,
) {
  return (
    Math.max(
      -1,
      ...mockStore.savedRequests
        .filter(
          (request) =>
            request.workspaceId === workspaceId &&
            request.collectionId === collectionId &&
            request.parentFolderId === parentFolderId,
        )
        .map((request) => request.sortOrder),
    ) + 1
  );
}

export function descendantFolderIds(workspaceId: string, folderId: string) {
  const ids = new Set<string>();
  let frontier = [folderId];
  while (frontier.length) {
    const next: string[] = [];
    for (const id of frontier) {
      if (ids.has(id)) continue;
      ids.add(id);
      next.push(
        ...mockStore.collectionFolders
          .filter(
            (folder) =>
              folder.workspaceId === workspaceId && folder.parentFolderId === id,
          )
          .map((folder) => folder.id),
      );
    }
    frontier = next;
  }
  return ids;
}

export function mockEnvList(workspaceId: string) {
  return mockStore.environments
    .filter((env) => env.workspaceId === workspaceId)
    .sort((a, b) => a.name.localeCompare(b.name));
}

function normalizeMockEnvironmentName(name: string) {
  return name.trim().toLowerCase();
}

export function assertMockEnvironmentNameAvailable(
  workspaceId: string,
  name: string,
  excludeId?: string,
) {
  const normalized = normalizeMockEnvironmentName(name);
  if (
    normalized &&
    mockStore.environments.some(
      (env) =>
        env.workspaceId === workspaceId &&
        env.id !== excludeId &&
        normalizeMockEnvironmentName(env.name) === normalized,
    )
  ) {
    throw new Error(`environment name already exists in this workspace: ${name}`);
  }
}

export function mockActiveEnvVariables(workspaceId: string): KeyValue[] {
  return (
    mockStore.environments.find(
      (env) => env.workspaceId === workspaceId && env.isActive,
    )?.variables ?? []
  );
}

export function trimMockSshHistory(sessionId: string) {
  const sessionEvents = mockStore.sshEvents.filter(
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
    const index = mockStore.sshEvents.indexOf(removed);
    if (index >= 0) mockStore.sshEvents.splice(index, 1);
  }
}

export function getMockLayout(workspaceId: string): WorkspaceLayout {
  mockStore.layouts[workspaceId] ??= {
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
    sidebarWidth: 248,
    bottomPanelHeight: 220,
    rightInspectorWidth: 300,
    updatedAt: new Date().toISOString(),
  };

  return mockStore.layouts[workspaceId];
}
