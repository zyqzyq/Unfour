import type { FlowDefinition, WorkspaceEnvironment, WorkspaceVariable } from "@unfour/command-client";
import { authoringProblems } from "./authoringValidation";
import { apiEnvironmentErrors } from "./apiEnvironmentValidation";
import { guaranteedUpstream, unsafeReferences } from "./referenceGraph";
import { environmentInputErrors, inputDefaults, inputDefinitionErrors, inputErrors, resourceErrors } from "./model";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowWorkspaceData } from "./useFlowWorkspaceData";

type ValidationEditor = Pick<FlowEditor, "invalid" | "environmentId" | "inputs" | "secretInputNames">;

function editorValidation(draft: FlowDefinition, invalid: FlowEditor["invalid"], data: FlowWorkspaceData) {
  const schemaProblems = inputDefinitionErrors(draft.inputs);
  const referenceProblems = unsafeReferences(draft.steps);
  const upstream = guaranteedUpstream(draft.steps);
  const boundsProblems = authoringProblems(draft);
  const resourceProblems = data.resourcesQuery.data ? resourceErrors(draft, data.resources) : [];
  const invalidEditor = boundsProblems.length > 0 || referenceProblems.length > 0
    || resourceProblems.some((problem) => ["flow.sqlRequired", "flow.sqlSingleStatement", "flow.sshMissingInputs", "flow.inputTypeError"].includes(problem.key))
    || schemaProblems.length > 0 || Object.entries(invalid).some(([key, value]) => !key.startsWith("run:") && value);
  return { schemaProblems, referenceProblems, upstream, boundsProblems, resourceProblems, invalidEditor };
}

function environmentVariables(workspace: WorkspaceVariable[], environments: WorkspaceEnvironment[], environmentId: string) {
  const selected = environments.find((env) => env.id === environmentId)?.variables ?? [];
  const enabled = [...workspace, ...selected].filter((variable) => variable.isEnabled && !variable.deletedAt);
  const environmentKeys = [...new Set([...workspace, ...environments.flatMap((env) => env.variables ?? [])]
    .filter((variable) => variable.isEnabled && !variable.deletedAt).map((variable) => variable.key))];
  // SSH defaults normalize keys and let the selected environment win. API
  // templates and reference suggestions retain the original key spelling.
  const defaults = new Map(enabled.map((variable) => [variable.key.trim().toLowerCase(), variable.value]));
  const effectiveApiKeys = new Set(enabled.map((variable) => variable.key));
  return { environmentKeys, defaults, effectiveApiKeys };
}

function environmentRequirements(draft: FlowDefinition) {
  const actions = draft.steps.map((step) => step.kind === "action" ? step.action : step.kind === "poll" || step.kind === "waitUntil" ? step.probe : null);
  const needsWorkspaceDefaults = actions.some((action) => action?.capability === "ssh" && action.arguments.workspaceDefaults === true);
  const hasApi = actions.some((action) => action?.capability === "api");
  return { needsWorkspaceDefaults, hasApi };
}

function environmentResourceProblems(draft: FlowDefinition, resources: FlowWorkspaceData["resources"], variables: ReturnType<typeof environmentVariables>, ready: boolean) {
  if (!ready) return { environmentInputProblems: [], apiProblems: [] };
  return {
    environmentInputProblems: environmentInputErrors(draft, resources, variables.defaults),
    apiProblems: apiEnvironmentErrors(draft, resources, variables.effectiveApiKeys),
  };
}

function environmentValidation(draft: FlowDefinition, environmentId: string, data: FlowWorkspaceData) {
  const { workspaceVariables, environments, resourcesQuery, resources } = data;
  const variables = environmentVariables(workspaceVariables.data ?? [], environments.data ?? [], environmentId);
  const { environmentKeys } = variables;
  const { needsWorkspaceDefaults, hasApi } = environmentRequirements(draft);
  const defaultsReady = workspaceVariables.isSuccess && !environments.isPending;
  const { environmentInputProblems, apiProblems } = environmentResourceProblems(draft, resources, variables, Boolean(resourcesQuery.data && defaultsReady));
  const environmentMissing = Boolean(environmentId) && !environments.data?.some((item) => item.id === environmentId);
  const invalidEnvironment = ((hasApi || needsWorkspaceDefaults) && !defaultsReady)
    || apiProblems.length > 0 || environmentInputProblems.length > 0 || environments.isError || environmentMissing;
  return { environmentKeys, needsWorkspaceDefaults, hasApi, environmentInputProblems, apiProblems, invalidEnvironment };
}

export function getFlowPageValidation(draft: FlowDefinition, editor: ValidationEditor, data: FlowWorkspaceData) {
  const editing = editorValidation(draft, editor.invalid, data);
  const environment = environmentValidation(draft, editor.environmentId, data);
  const resolvedInputs = { ...inputDefaults(draft.inputs), ...editor.inputs };
  const inputProblems = inputErrors(draft.inputs, resolvedInputs);
  const manualSecrets = editor.secretInputNames.split(",").map((name) => name.trim()).filter(Boolean);
  const manualSecretErrors = manualSecrets.filter((name) => !Object.prototype.hasOwnProperty.call(resolvedInputs, name));
  const invalidRun = environment.invalidEnvironment || manualSecretErrors.length > 0 || editing.invalidEditor
    || inputProblems.length > 0 || editing.resourceProblems.length > 0 || Object.values(editor.invalid).some(Boolean)
    || !data.resourcesQuery.data || data.resourcesQuery.isError;
  return { ...editing, ...environment, resolvedInputs, inputProblems, manualSecrets, manualSecretErrors, invalidRun };
}

export type FlowPageValidation = ReturnType<typeof getFlowPageValidation>;
