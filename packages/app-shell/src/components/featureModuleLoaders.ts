import type { QueryClient } from "@tanstack/react-query";
import type { WorkspaceTab } from "@unfour/command-client";

function cachedLoader<T>(load: () => Promise<T>) {
  let pending: Promise<T> | null = null;
  return () => {
    if (!pending) {
      pending = load().catch((error) => {
        pending = null;
        throw error;
      });
    }
    return pending;
  };
}

export const loadApiClientModule = cachedLoader(() => import("@unfour/api-client"));
export const loadDatabaseModule = cachedLoader(() => import("@unfour/database"));
export const loadSshTerminalModule = cachedLoader(() => import("@unfour/ssh-terminal"));
export const loadWorkspaceEnvironmentsModule = cachedLoader(
  () => import("@unfour/workspace-environments"),
);

export type FeatureModulePreloadContext = {
  queryClient: QueryClient;
  workspaceId: string;
};

export async function preloadFeatureModule(
  kind: WorkspaceTab["kind"],
  context?: FeatureModulePreloadContext,
): Promise<unknown> {
  if (kind === "api") return loadApiClientModule();
  if (kind === "database") return loadDatabaseModule();
  const module = await loadSshTerminalModule();
  if (context) {
    await module.preloadSshWorkspace(context.queryClient, context.workspaceId);
  }
  return module;
}
