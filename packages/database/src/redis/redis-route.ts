import { connectionTemporaryHandle, type RedisConnection } from "@unfour/command-client";
import type { TFunction } from "@unfour/ui";

export function redisRouteLabel(connection: RedisConnection, t: TFunction) {
  const handle = connectionTemporaryHandle("redis", connection.workspaceId, connection.id);
  if (handle) return t("network.temporarySession", { id: handle.slice(-8) });
  if (!connection.sshConnectionId) return t("redis.route.direct");
  if (connection.routeNeedsReview) return t("network.errors.SSH_TUNNEL_ROUTE_CHANGED");
  return connection.sshConnectionName
    ? t("redis.route.via", { name: connection.sshConnectionName })
    : t("redis.route.missing");
}
