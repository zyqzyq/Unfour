import type {
  SshTaskLocalBinding,
  SshTaskSaveInput,
  SshTaskStepInput,
  SshTaskStepType,
  SshTaskDetail,
} from "@unfour/command-client";
import type { TFunction } from "@unfour/ui";

const TEMPLATE_FIELDS: Record<SshTaskStepType, string[]> = {
  command: ["command", "workingDirectory"],
  upload: ["localPath", "remotePath"],
  download: ["remotePath", "localPath"],
};

const PLACEHOLDER = /\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}/g;

export function preferredTaskConnectionId(
  binding: SshTaskLocalBinding | null,
): string {
  return binding?.defaultConnectionId ?? binding?.lastUsedConnectionId ?? "";
}

export function detectTaskInputs(steps: SshTaskStepInput[], enabledOnly = false) {
  const inputs: string[] = [];
  for (const step of steps) {
    if (enabledOnly && !step.enabled) continue;
    const config = step.configJson as unknown as Record<string, unknown>;
    for (const field of TEMPLATE_FIELDS[step.stepType]) {
      const value = config[field];
      if (typeof value !== "string") continue;
      for (const match of value.matchAll(PLACEHOLDER)) {
        const variable = match[1];
        if (!inputs.includes(variable)) inputs.push(variable);
      }
    }
  }
  return inputs;
}

export function createTaskStep(
  stepType: SshTaskStepType,
  position: number,
  t: TFunction,
): SshTaskStepInput {
  return {
    name: t("ssh.tasks.editor.defaultStepName", { type: t(`ssh.tasks.stepTypes.${stepType}`), position: position + 1 }),
    stepType,
    position,
    enabled: true,
    configVersion: 1,
    configJson:
      stepType === "command"
        ? {
            command: "",
            workingDirectory: "",
            timeoutSeconds: 300,
            continueOnError: false,
          }
        : stepType === "upload"
          ? { localPath: "", remotePath: "", overwrite: true }
          : { remotePath: "", localPath: "", overwrite: true },
  };
}

export function dockerImageExportTemplate(workspaceId: string, t: TFunction): SshTaskSaveInput {
  const commands = [
    ["pullImage", "docker pull {{source_image}}"],
    ["tagImage", "docker tag {{source_image}} {{target_image}}"],
    [
      "saveImage",
      "docker save {{target_image}} -o /tmp/{{archive_name}}.tar",
    ],
  ] as const;
  const steps: SshTaskStepInput[] = commands.map(([name, command], position) => ({
    ...createTaskStep("command", position, t),
    name: t(`ssh.tasks.dockerExample.${name}`),
    configJson: {
      command,
      workingDirectory: "",
      timeoutSeconds: 300,
      continueOnError: false,
    },
  }));
  steps.push({
    ...createTaskStep("download", 3, t),
    name: t("ssh.tasks.dockerExample.downloadArchive"),
    configJson: {
      remotePath: "/tmp/{{archive_name}}.tar",
      localPath: "{{local_output_dir}}/{{archive_name}}.tar",
      overwrite: true,
    },
  });
  steps.push({
    ...createTaskStep("command", 4, t),
    name: t("ssh.tasks.dockerExample.removeArchive"),
    configJson: {
      command: "rm -f /tmp/{{archive_name}}.tar",
      workingDirectory: "",
      timeoutSeconds: 300,
      continueOnError: false,
    },
  });
  return {
    workspaceId,
    name: t("ssh.tasks.dockerExample.name"),
    description: t("ssh.tasks.dockerExample.description"),
    defaultConnectionId: null,
    steps,
  };
}

export function duplicateTaskStep(
  steps: SshTaskStepInput[],
  index: number,
  t: TFunction,
): SshTaskStepInput[] {
  const source = steps[index];
  if (!source) return steps;
  const copy: SshTaskStepInput = {
    ...source,
    id: undefined,
    name: t("ssh.tasks.editor.stepCopyName", { name: source.name }),
    configJson: { ...source.configJson },
  };
  return normalizePositions([
    ...steps.slice(0, index + 1),
    copy,
    ...steps.slice(index + 1),
  ]);
}

export function moveTaskStep(
  steps: SshTaskStepInput[],
  index: number,
  direction: -1 | 1,
): SshTaskStepInput[] {
  return reorderTaskStep(steps, index, index + direction);
}

export function reorderTaskStep(
  steps: SshTaskStepInput[],
  fromIndex: number,
  toIndex: number,
): SshTaskStepInput[] {
  if (
    fromIndex === toIndex ||
    fromIndex < 0 ||
    toIndex < 0 ||
    fromIndex >= steps.length ||
    toIndex >= steps.length
  ) {
    return steps;
  }
  const copy = steps.slice();
  const [item] = copy.splice(fromIndex, 1);
  copy.splice(toIndex, 0, item);
  return normalizePositions(copy);
}

export function removeTaskStep(steps: SshTaskStepInput[], index: number) {
  return normalizePositions(steps.filter((_, itemIndex) => itemIndex !== index));
}

export function normalizePositions(steps: SshTaskStepInput[]) {
  return steps.map((step, position) => ({ ...step, position }));
}

export function taskDetailToDraft(detail: SshTaskDetail): SshTaskSaveInput {
  return {
    id: detail.task.id,
    workspaceId: detail.task.workspaceId,
    name: detail.task.name,
    description: detail.task.description,
    defaultConnectionId: detail.localBinding?.defaultConnectionId ?? null,
    steps: detail.steps.map((step) => ({
      id: step.id,
      name: step.name,
      stepType: step.stepType,
      position: step.position,
      enabled: step.enabled,
      configVersion: step.configVersion,
      configJson: { ...step.configJson },
    })),
  };
}
