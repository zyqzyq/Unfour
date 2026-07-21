import type {
  SshTaskLocalBinding,
  SshTaskSaveInput,
  SshTaskStepInput,
  SshTaskStepType,
} from "@unfour/command-client";

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
): SshTaskStepInput {
  return {
    name: `${stepType[0].toUpperCase()}${stepType.slice(1)} ${position + 1}`,
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

export function dockerImageExportTemplate(workspaceId: string): SshTaskSaveInput {
  const commands = [
    ["Pull image", "docker pull {{source_image}}"],
    ["Tag image", "docker tag {{source_image}} {{target_image}}"],
    [
      "Save image",
      "docker save {{target_image}} -o /tmp/{{archive_name}}.tar",
    ],
  ] as const;
  const steps: SshTaskStepInput[] = commands.map(([name, command], position) => ({
    ...createTaskStep("command", position),
    name,
    configJson: {
      command,
      workingDirectory: "",
      timeoutSeconds: 300,
      continueOnError: false,
    },
  }));
  steps.push({
    ...createTaskStep("download", 3),
    name: "Download archive",
    configJson: {
      remotePath: "/tmp/{{archive_name}}.tar",
      localPath: "{{local_output_dir}}/{{archive_name}}.tar",
      overwrite: true,
    },
  });
  steps.push({
    ...createTaskStep("command", 4),
    name: "Remove remote archive",
    configJson: {
      command: "rm -f /tmp/{{archive_name}}.tar",
      workingDirectory: "",
      timeoutSeconds: 300,
      continueOnError: false,
    },
  });
  return {
    workspaceId,
    name: "Docker Image Export",
    description: "Pull, retag, save, and download a Docker image for offline use.",
    defaultConnectionId: null,
    steps,
  };
}

export function duplicateTaskStep(
  steps: SshTaskStepInput[],
  index: number,
): SshTaskStepInput[] {
  const source = steps[index];
  if (!source) return steps;
  const copy: SshTaskStepInput = {
    ...source,
    id: undefined,
    name: `${source.name} Copy`,
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
  const target = index + direction;
  if (index < 0 || target < 0 || index >= steps.length || target >= steps.length) {
    return steps;
  }
  const copy = steps.slice();
  [copy[index], copy[target]] = [copy[target], copy[index]];
  return normalizePositions(copy);
}

export function removeTaskStep(steps: SshTaskStepInput[], index: number) {
  return normalizePositions(steps.filter((_, itemIndex) => itemIndex !== index));
}

export function normalizePositions(steps: SshTaskStepInput[]) {
  return steps.map((step, position) => ({ ...step, position }));
}
