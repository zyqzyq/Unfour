import { describe, expect, it } from "vitest";
import type { SshTaskRun } from "@unfour/command-client";
import { restoreTaskRunInputs, taskRunDefaults } from "./task-run-context";

const variable = (key: string, value: string, isSecret = false) => ({ key, value, isSecret, isEnabled: true });
const environment = { id: "env", isActive: false, variables: [variable("region", "new-env"), variable("custom", "rotated", true)] };
const variables = [variable("region", "workspace"), variable("version", "current")];
const run: SshTaskRun = {
  id: "past", workspaceId: "w", taskId: "t", connectionId: "old", status: "failed", startedAt: "", finishedAt: "", logPath: "", errorMessage: "failed",
  context: { version: 1, environmentId: "env", taskVersion: "old", environmentUpdatedAt: "old", inputs: {
    region: { source: "environment" }, custom: { source: "environment" }, version: { source: "manual", value: "saved" },
    credential: { source: "manual", value: "unsafe-old-value" }, privateValue: { source: "manual" },
  } },
};
const names = ["region", "custom", "version", "credential", "privateValue", "newInput"];

describe("task run recall", () => {
  it("loads current defaults on ordinary run and restores only manual non-sensitive values", () => {
    expect(taskRunDefaults(names, variables, environment).inputs.version).toBe("current");
    const restored = restoreTaskRunInputs(names, variables, environment, run);
    expect(restored.inputs).toEqual({ region: "new-env", custom: "rotated", version: "saved", credential: "", privateValue: "", newInput: "" });
    expect(restored.secretNames).toEqual(expect.arrayContaining(["custom", "credential", "privateValue"]));
    expect(restored.filledFromWorkspace).toEqual(["region", "custom"]);
  });
  it("does not fall back to workspace when an environment input disappears", () => {
    expect(restoreTaskRunInputs(names, variables, null, run).inputs.region).toBe("");
    expect(restoreTaskRunInputs(names, variables, { ...environment, variables: [] }, run).inputs.region).toBe("");
  });
  it("preserves secret provenance after a variable disappears or is overridden by a public environment value", () => {
    const previous = { ...run, context: { ...run.context!, inputs: {
      custom: { source: "environment" as const, secret: true }, region: { source: "workspace" as const },
    } } };
    const restored = restoreTaskRunInputs(names, [variable("region", "workspace-private", true)], { ...environment, variables: [variable("region", "public")] }, previous);
    expect(restored.inputs.custom).toBe("");
    expect(restored.inputs.region).toBe("workspace-private");
    expect(restored.secretNames).toEqual(expect.arrayContaining(["custom", "region"]));
  });
  it("uses current defaults for legacy histories and explicitly re-resolves workspace sources", () => {
    expect(restoreTaskRunInputs(names, variables, environment, { ...run, context: null }).inputs.version).toBe("current");
    const saved = { ...run, context: { ...run.context!, inputs: { region: { source: "workspace" as const } } } };
    expect(restoreTaskRunInputs(names, variables, environment, saved).inputs.region).toBe("workspace");
  });
});
