import type { ReactNode } from "react";
import { useWorkspaceBundleExchange } from "./WorkspaceBundleExchange";
import { WorkspaceBundleContext } from "./workspace-bundle-context";

/** Menu and command-palette entry points share one local exchange and dialog. */
export function WorkspaceBundleExchangeProvider({ children, onImported }: {
  children: ReactNode;
  onImported: (workspaceId: string) => void;
}) {
  const { dialog, ...actions } = useWorkspaceBundleExchange(onImported);
  return <WorkspaceBundleContext.Provider value={actions}>
    {children}
    {dialog}
  </WorkspaceBundleContext.Provider>;
}
