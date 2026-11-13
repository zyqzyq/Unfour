import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { getApiRoutes, listSshConnections, setApiEnvironmentRoute, type ApiRoute, type ApiRouteEdit, type SshConnection } from "@unfour/command-client";
import type { ApiRequestTab } from "../model/request-tabs";

type Routes = Awaited<ReturnType<typeof getApiRoutes>>;
export const routeQueryKey = (workspaceId: string, requestId?: string | null, environmentId?: string | null) =>
  ["api-routes", workspaceId, requestId ?? null, environmentId ?? null] as const;
export const environmentRouteMutationKey = (workspaceId: string) => ["api-environment-route", workspaceId] as const;

export function effectiveApiRoute(routes: Routes, draft?: ApiRoute): ApiRoute {
  if (!draft) return routes.effective;
  return draft.mode === "inherit" ? routes.environment : draft;
}

export function useApiRoutes(workspaceId: string, tab: ApiRequestTab | null, environmentId: string | null, environmentStatus: "pending" | "error" | "success") {
  const environmentReady = environmentStatus === "success";
  const client = useQueryClient();
  const query = useQuery({
    enabled: Boolean(workspaceId && tab && environmentReady),
    queryKey: routeQueryKey(workspaceId, tab?.savedRequestId, environmentId),
    queryFn: () => getApiRoutes(workspaceId, tab?.savedRequestId, environmentId),
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["environments", "api-requests", "ssh-connections"] },
  });
  const ssh = useQuery({
    enabled: Boolean(workspaceId && tab),
    queryKey: ["ssh-connections", workspaceId],
    queryFn: () => listSshConnections(workspaceId),
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["ssh-connections"] },
  });
  const environmentMutation = useMutation({
    mutationKey: environmentRouteMutationKey(workspaceId),
    mutationFn: ({ id, edit }: { id: string; edit: ApiRouteEdit }) => setApiEnvironmentRoute(workspaceId, id, edit),
    onSuccess: () => client.invalidateQueries({ queryKey: ["api-routes", workspaceId] }),
  });
  const view = routePreview(query, ssh, tab?.draft.route?.route, environmentId, environmentStatus);
  return { query, ssh, ...view, ready: view.ready && !environmentMutation.isPending, environmentMutation };
}

function routePreview(
  query: UseQueryResult<Routes>,
  ssh: UseQueryResult<SshConnection[]>,
  draft: ApiRoute | undefined,
  environmentId: string | null,
  environmentStatus: "pending" | "error" | "success",
) {
  const routes = query.data;
  const effective = routes && effectiveApiRoute(routes, draft);
  const sshId = effective?.mode === "ssh" ? effective.sshConnectionId : null;
  const selectedHost = ssh.data?.find((item) => item.id === sshId);
  const hostReady = sshId === null || Boolean(selectedHost && ssh.isSuccess);
  const ready = environmentStatus === "success" && query.isSuccess && query.data.environmentId === environmentId && hostReady;
  const pending = environmentStatus === "pending" || query.isLoading || Boolean(sshId && ssh.isLoading);
  return { routes, effective, selectedHost, ready, pending };
}

export type ApiRouteControls = ReturnType<typeof useApiRoutes>;
