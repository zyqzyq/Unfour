import { describe, expect, it } from "vitest";
import {
  detectTaskInputs,
  dockerImageExportTemplate,
  duplicateTaskStep,
  moveTaskStep,
  preferredTaskConnectionId,
  removeTaskStep,
} from "./task-template";

describe("SSH task editor logic", () => {
  it("detects supported placeholders once in first-seen order", () => {
    const task = dockerImageExportTemplate("workspace");
    expect(detectTaskInputs(task.steps)).toEqual([
      "source_image",
      "target_image",
      "archive_name",
      "local_output_dir",
    ]);
  });

  it("ignores placeholders in unsupported config fields and disabled steps on run", () => {
    const task = dockerImageExportTemplate("workspace");
    task.steps[0] = {
      ...task.steps[0],
      enabled: false,
      configJson: {
        ...task.steps[0].configJson,
        command: "echo {{disabled_value}}",
        ignored: "{{not_scanned}}",
      } as typeof task.steps[0]["configJson"],
    };
    expect(detectTaskInputs(task.steps, true)).not.toContain("disabled_value");
    expect(detectTaskInputs(task.steps)).not.toContain("not_scanned");
  });

  it("duplicates, moves, removes, and renumbers steps deterministically", () => {
    const original = dockerImageExportTemplate("workspace").steps.slice(0, 2);
    original[0] = { ...original[0], id: "persisted-step" };
    const duplicated = duplicateTaskStep(original, 0);
    expect(duplicated.map((step) => step.position)).toEqual([0, 1, 2]);
    expect(duplicated[1].name).toContain("Copy");
    expect(duplicated[1].id).toBeUndefined();
    expect(duplicated[1].configVersion).toBe(1);

    const moved = moveTaskStep(duplicated, 1, 1);
    expect(moved[2].id).toBeUndefined();
    expect(removeTaskStep(moved, 0).map((step) => step.position)).toEqual([0, 1]);
  });

  it("requires selection without a local binding and otherwise prefers the default", () => {
    expect(preferredTaskConnectionId(null)).toBe("");
    const binding = {
      taskId: "task",
      workspaceId: "workspace",
      defaultConnectionId: "default",
      lastUsedConnectionId: "last-used",
      createdAt: "now",
      updatedAt: "now",
    };
    expect(preferredTaskConnectionId(binding)).toBe("default");
    expect(
      preferredTaskConnectionId({ ...binding, defaultConnectionId: null }),
    ).toBe("last-used");
  });
});
