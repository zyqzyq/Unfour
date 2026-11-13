import { QueryClient } from "@tanstack/react-query";
import { expect, it } from "vitest";
import type { RedisConnection } from "@unfour/command-client";
import { redisConnectionsQueryKey, updateRedisConnectionsAfterMutation } from "./redis-connections";

it("discards an older listing and preserves unrelated or newer saved revisions", async () => {
  const client = new QueryClient();
  const cache: RedisConnection = { id: "cache", workspaceId: "w", name: "Cache", host: "localhost",
    port: 6379, dbIndex: 0, username: null, tls: false, revision: 2, hasPassword: false };
  const other = { ...cache, id: "other" };
  const queryKey = redisConnectionsQueryKey("w");
  client.setQueryData(queryKey, [other, cache]);
  let finish!: (records: RedisConnection[]) => void;
  const delayed = new Promise<RedisConnection[]>((resolve) => { finish = resolve; });
  const oldRead = client.fetchQuery({ queryKey, queryFn: () => delayed }).catch(() => {});
  await updateRedisConnectionsAfterMutation(client, { ...cache, revision: 1 }, "save");
  finish([]);
  await oldRead;
  expect(client.getQueryData(queryKey)).toEqual([other, cache]);
  await updateRedisConnectionsAfterMutation(client, cache, "delete");
  expect(client.getQueryData(queryKey)).toEqual([other]);
  expect(client.getQueryState(queryKey)?.isInvalidated).toBe(true);
  client.clear();
});
