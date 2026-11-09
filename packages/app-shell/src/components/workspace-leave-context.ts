import { createContext } from "react";

// Lifecycle commands such as create/delete change the workspace in the backend
// themselves. They must pass the same leave gate before executing the command.
export const WorkspaceLeaveContext = createContext<(action: () => void) => void>((action) => action());
