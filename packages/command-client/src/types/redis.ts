export interface RedisConnectionInput {
  id?: string | null;
  workspaceId: string;
  name: string;
  host: string;
  port: number;
  username: string | null;
  dbIndex: number;
  tls: boolean;
  /** Device-local desktop grant. Missing on older inputs. */
  allowWrites?: boolean;
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

export type RedisCollectionKind = "hash" | "list" | "set" | "zset";
interface RedisKeyTarget {
  workspaceId: string;
  connectionId: string;
  expectedRevision: number;
  keyBase64: string;
}
export interface RedisCollectionRequest extends RedisKeyTarget {
  kind: RedisCollectionKind;
  cursor: string;
  pattern: string;
  descending: boolean;
  startIndex?: number;
}
export interface RedisMutationRequest extends RedisKeyTarget {
  mutation: RedisMutation;
  confirmed: boolean;
}

export interface RedisCollectionRow {
  memberBase64: string;
  valueBase64: string | null;
  index: number | null;
  score: string | null;
}
export interface RedisCollectionPage {
  cursor: string;
  rows: RedisCollectionRow[];
}

export type RedisMutation =
  | { kind: "createString"; valueBase64: string; ttlMs?: number }
  | { kind: "updateString"; expectedBase64: string; valueBase64: string }
  | { kind: "createHash" | "addHashField"; fieldBase64: string; valueBase64: string }
  | { kind: "updateHashField"; fieldBase64: string; expectedBase64: string; valueBase64: string }
  | { kind: "deleteHashField"; fieldBase64: string; expectedBase64: string }
  | { kind: "setTtl"; expectedType: string; ttlMs: number }
  | { kind: "persist" | "deleteKey"; expectedType: string };

/** In-memory handoff between a Redis key tab and a new Flow draft. */
export interface RedisFlowIntent {
  workspaceId: string;
  connectionId: string;
  key: string;
}
