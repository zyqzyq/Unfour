import type { FlowAction } from "@unfour/command-client";
import type { Resources } from "./model";
import type { Variable } from "./variables";
export type ActionEditorProps = {
  action: FlowAction; resources: Resources; variables: Variable[];
  onChange: (action: FlowAction) => void; onValidity: (field: string, valid: boolean) => void;
};
