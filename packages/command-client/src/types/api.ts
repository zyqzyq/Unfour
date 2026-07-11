export type KeyValue = {
  key: string;
  value: string;
  enabled: boolean;
};

export type ApiEnvironment = {
  id: string;
  workspaceId: string;
  name: string;
  variables: KeyValue[];
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ApiCollection = {
  id: string;
  workspaceId: string;
  name: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ApiCollectionFolder = {
  id: string;
  workspaceId: string;
  collectionId: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
};

export type ApiRequestInput = {
  workspaceId: string;
  name?: string;
  parentFolderId?: string | null;
  collectionId?: string | null;
  authJson?: string;
  method: string;
  url: string;
  headers: KeyValue[];
  query: KeyValue[];
  body?: string;
  bodyKind: string;
  timeoutMs?: number;
};

export type ApiResponse = {
  historyId: string;
  status: number;
  statusText: string;
  headers: KeyValue[];
  body: string;
  durationMs: number;
};

export type ApiHistoryItem = {
  id: string;
  workspaceId: string;
  name: string | null;
  method: string;
  url: string;
  status: number | null;
  durationMs: number | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type ApiHistoryDetail = {
  id: string;
  workspaceId: string;
  name: string | null;
  method: string;
  url: string;
  requestHeadersJson: string;
  requestQueryJson: string;
  requestBody: string | null;
  status: number | null;
  durationMs: number | null;
  responseHeadersJson: string;
  responseBodyPreview: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type ApiSavedRequest = {
  id: string;
  workspaceId: string;
  name: string;
  collectionId: string;
  parentFolderId: string | null;
  sortOrder: number;
  authJson?: string;
  method: string;
  url: string;
  headersJson: string;
  queryJson: string;
  body: string | null;
  bodyKind: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};
