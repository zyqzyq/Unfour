import { describe, expect, it } from "vitest";
import { createTranslator } from "@unfour/ui";
import {
  createTaskStep,
  detectTaskInputs,
  dockerImageExportTemplate,
  duplicateTaskStep,
  moveTaskStep,
  preferredTaskConnectionId,
  removeTaskStep,
  reorderTaskStep,
} from "./task-template";

const t = createTranslator("en");

describe("SSH task editor logic", () => {
  it("detects supported placeholders once in first-seen order", () => {
    const task = dockerImageExportTemplate("workspace", t);
    expect(detectTaskInputs(task.steps)).toEqual([
      "source_image",
      "target_image",
      "archive_name",
      "local_output_dir",
    ]);
  });

  it("ignores placeholders in unsupported config fields and disabled steps on run", () => {
    const task = dockerImageExportTemplate("workspace", t);
    task.steps[0] = {
      ...task.steps[0],
      enabled: false,
      configJson: {
        ...task.steps[0].configJson,
        command: "echo {{disabled_value}}",
        ignored: "{{not_scanned}}",
      } as unknown as typeof task.steps[0]["configJson"],
    };
    expect(detectTaskInputs(task.steps, true)).not.toContain("disabled_value");
    expect(detectTaskInputs(task.steps)).not.toContain("not_scanned");
  });

  it("duplicates, moves, removes, and renumbers steps deterministically", () => {
    const original = dockerImageExportTemplate("workspace", t).steps.slice(0, 2);
    original[0] = { ...original[0], id: "persisted-step" };
    const duplicated = duplicateTaskStep(original, 0, t);
    expect(duplicated.map((step) => step.position)).toEqual([0, 1, 2]);
    expect(duplicated[1].name).toContain("Copy");
    expect(duplicated[1].id).toBeUndefined();
    expect(duplicated[1].configVersion).toBe(1);

    const moved = moveTaskStep(duplicated, 1, 1);
    expect(moved[2].id).toBeUndefined();
    expect(removeTaskStep(moved, 0).map((step) => step.position)).toEqual([0, 1]);
  });

  it("reorders steps to an absolute index", () => {
    const steps = dockerImageExportTemplate("workspace", t).steps.slice(0, 3);
    steps[0] = { ...steps[0], id: "a" };
    steps[1] = { ...steps[1], id: "b" };
    steps[2] = { ...steps[2], id: "c" };
    const reordered = reorderTaskStep(steps, 0, 2);
    expect(reordered.map((step) => step.id)).toEqual(["b", "c", "a"]);
    expect(reordered.map((step) => step.position)).toEqual([0, 1, 2]);
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

  it("localizes generated copy without changing commands, paths, inputs or source names", () => {
    const en = dockerImageExportTemplate("workspace", t);
    const zh = dockerImageExportTemplate("workspace", createTranslator("zh-CN"));
    expect(en.name).toBe("Docker Image Export");
    expect(zh.name).toBe("Docker 镜像导出");
    expect(zh.description).toBe("拉取、重新标记、保存并下载 Docker 镜像，供离线使用。");
    expect(zh.steps.map((step) => step.name)).toEqual(["拉取镜像", "标记镜像", "保存镜像", "下载归档", "删除远程归档"]);
    expect(zh.steps.map((step) => step.configJson)).toEqual(en.steps.map((step) => step.configJson));
    expect(detectTaskInputs(zh.steps)).toEqual(detectTaskInputs(en.steps));
    const original = [{ ...createTaskStep("command", 0, t), name: "Custom name" }];
    expect(duplicateTaskStep(original, 0, createTranslator("zh-CN"))[1].name).toBe("Custom name 副本");
    expect(duplicateTaskStep(original, 0, t)[1].name).toBe("Custom name Copy");
    expect(original[0].name).toBe("Custom name");
  });

  it("uses translated step types and interpolates the generated step number", () => {
    const localized = createTranslator("zh-CN", { "zh-CN": { ssh: { tasks: { stepTypes: { command: "命令" } } } } });
    expect(createTaskStep("command", 1, localized).name).toBe("命令 2");
    expect(createTaskStep("upload", 0, t).name).toBe("Upload 1");
    expect(createTaskStep("download", 2, t).name).toBe("Download 3");
  });
});
