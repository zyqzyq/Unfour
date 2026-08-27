import { createContext, useContext } from "react";
import type { UpdateContextValue } from "./updateTypes";

export const UpdateContext = createContext<UpdateContextValue | null>(null);

export function useUpdate(): UpdateContextValue {
  const value = useContext(UpdateContext);
  if (!value) throw new Error("useUpdate must be used inside UpdateProvider");
  return value;
}
