import type { RedisConnection } from "@unfour/command-client";
import type { TFunction } from "@unfour/ui";

export function redisRouteLabel(connection: RedisConnection, t: TFunction) {
  if (!connection.sshConnectionId) return t("redis.route.direct");
  if (connection.routeNeedsReview) return t("network.errors.SSH_TUNNEL_ROUTE_CHANGED");
  return connection.sshConnectionName
    ? t("redis.route.via", { name: connection.sshConnectionName })
    : t("redis.route.missing");
}
