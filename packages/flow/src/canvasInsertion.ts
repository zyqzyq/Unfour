import { createContext } from "react";

export const CanvasInsertion = createContext<{ disabled: boolean; insert: (edgeId: string, kind: string) => void }>({ disabled: false, insert: () => {} });
