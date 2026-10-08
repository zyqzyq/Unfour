import { createContext, useContext } from "react";
import type { useWorkspaceBundleExchange } from "./WorkspaceBundleExchange";

type WorkspaceBundleActions = Omit<ReturnType<typeof useWorkspaceBundleExchange>, "dialog">;
export const WorkspaceBundleContext = createContext<WorkspaceBundleActions | null>(null);

export function useWorkspaceBundleActions() {
  const actions = useContext(WorkspaceBundleContext);
  if (!actions) throw new Error("WorkspaceBundleExchangeProvider is required");
  return actions;
}
