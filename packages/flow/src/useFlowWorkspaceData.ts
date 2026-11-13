import { useQuery } from "@tanstack/react-query";
import {
  getSshTask,
  listDatabaseConnections,
  listRedisConnections,
  listFlows,
  listSavedApiRequests,
  listSshConnections,
  listSshTasks,
  listWorkspaceEnvironments,
  listWorkspaceVariables,
} from "@unfour/command-client";
import type { Resources } from "./model";

const emptyResources: Resources = { api: [], ssh: [], database: [], redis: [], connections: [] };

export function useFlowWorkspaceData(workspaceId: string) {
  const flows = useQuery({
    queryKey: ["flows", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["flows"] },
    queryFn: () => listFlows(workspaceId),
  });
  const resourcesQuery = useQuery({
    queryKey: ["flow-resources", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["api-requests", "ssh-tasks", "ssh-connections", "database-connections"] },
    queryFn: async (): Promise<Resources> => {
      const [api, ssh, database, redis, connections] = await Promise.all([
        listSavedApiRequests(workspaceId),
        listSshTasks(workspaceId),
        listDatabaseConnections(workspaceId),
        listRedisConnections(workspaceId),
        listSshConnections(workspaceId),
      ]);
      const detailedTasks = await Promise.all(ssh.map(async (task) => ({
        ...task, detail: await getSshTask(workspaceId, task.id),
      })));
      return { api, ssh: detailedTasks, database, redis, connections };
    },
  });
  const environments = useQuery({
    queryKey: ["workspace-environments", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["environments"] },
    queryFn: () => listWorkspaceEnvironments(workspaceId),
  });
  const workspaceVariables = useQuery({
    queryKey: ["flow-workspace-variables", workspaceId],
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["workspace-variables"] },
    queryFn: () => listWorkspaceVariables(workspaceId),
  });
  return { flows, resourcesQuery, environments, workspaceVariables, resources: resourcesQuery.data ?? emptyResources };
}

export type FlowWorkspaceData = ReturnType<typeof useFlowWorkspaceData>;
