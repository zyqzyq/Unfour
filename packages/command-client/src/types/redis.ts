export interface RedisConnectionInput {
  id?: string | null;
  workspaceId: string;
  name: string;
  host: string;
  port: number;
  username: string | null;
  dbIndex: number;
  tls: boolean;
  expectedRevision?: number | null;
}

export interface RedisConnection extends Omit<RedisConnectionInput, "id" | "expectedRevision"> {
  id: string;
  revision: number;
  hasPassword: boolean;
}

export interface RedisKeyInfo {
  exists: boolean;
  keyType: string;
  pttlMs: number;
  length: number | null;
  preview: { base64: string; truncated: boolean } | null;
  fieldFound: boolean | null;
}

export interface RedisScanPage {
  cursor: string;
  /** RESP bulk strings, encoded losslessly even for non-UTF-8 keys. */
  keys: string[];
}

/** In-memory handoff between a Redis key tab and a new Flow draft. */
export interface RedisFlowIntent {
  workspaceId: string;
  connectionId: string;
  key: string;
}
