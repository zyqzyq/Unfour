import { useQuery } from "@tanstack/react-query";
import {
  getSshTask,
  listDatabaseConnections,
  listFlows,
  listSavedApiRequests,
  listSshConnections,
  listSshTasks,
  listWorkspaceEnvironments,
  listWorkspaceVariables,
} from "@unfour/command-client";
import type { Resources } from "./model";

const emptyResources: Resources = { api: [], ssh: [], database: [], connections: [] };

export function useFlowWorkspaceData(workspaceId: string) {
  const flows = useQuery({
    queryKey: ["flows", workspaceId],
    queryFn: () => listFlows(workspaceId),
  });
  const resourcesQuery = useQuery({
    queryKey: ["flow-resources", workspaceId],
    queryFn: async (): Promise<Resources> => {
      const [api, ssh, database, connections] = await Promise.all([
        listSavedApiRequests(workspaceId),
        listSshTasks(workspaceId),
        listDatabaseConnections(workspaceId),
        listSshConnections(workspaceId),
      ]);
      const detailedTasks = await Promise.all(ssh.map(async (task) => ({
        ...task, detail: await getSshTask(workspaceId, task.id),
      })));
      return { api, ssh: detailedTasks, database, connections };
    },
  });
  const environments = useQuery({
    queryKey: ["workspace-environments", workspaceId],
    queryFn: () => listWorkspaceEnvironments(workspaceId),
  });
  const workspaceVariables = useQuery({
    queryKey: ["flow-workspace-variables", workspaceId],
    queryFn: () => listWorkspaceVariables(workspaceId),
  });
  return { flows, resourcesQuery, environments, workspaceVariables, resources: resourcesQuery.data ?? emptyResources };
}

export type FlowWorkspaceData = ReturnType<typeof useFlowWorkspaceData>;
