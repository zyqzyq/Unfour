import type { WorkspaceVariable, WorkspaceVariableInput } from "../../types";

// Shared record mechanics only. Callers own scope selection, environment
// metadata revisions, activation, and the collection being replaced.
export function createMockVariable(input: WorkspaceVariableInput, workspaceId: string, now: string): WorkspaceVariable {
  return {
    ...input,
    id: crypto.randomUUID(),
    workspaceId,
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
    revision: 1,
  };
}

export function updateMockVariable(variable: WorkspaceVariable, input: WorkspaceVariableInput, variableId: string) {
  Object.assign(variable, input, {
    id: variableId,
    updatedAt: new Date().toISOString(),
    revision: variable.revision + 1,
  });
  return variable;
}

export function replaceMockVariables(
  inputs: WorkspaceVariableInput[],
  current: WorkspaceVariable[],
  workspaceId: string,
  now: string,
): WorkspaceVariable[] {
  return inputs.map((input, sortOrder) => {
    const existing = current.find((variable) => variable.id === input.id);
    return {
      ...input,
      id: existing?.id ?? crypto.randomUUID(),
      workspaceId,
      sortOrder,
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
      revision: (existing?.revision ?? 0) + 1,
    };
  });
}
