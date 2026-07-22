import type { WorkspaceEnvironment } from "@unfour/command-client";

export function findDuplicateEnvironmentName(
  environments: Array<Pick<WorkspaceEnvironment, "id" | "name">>,
  name: string,
  excludeId?: string,
) {
  const normalized = normalizeEnvironmentName(name);
  if (!normalized) return null;
  return (
    environments.find(
      (environment) =>
        environment.id !== excludeId &&
        normalizeEnvironmentName(environment.name) === normalized,
    )?.name ?? null
  );
}

export function nextEnvironmentName(
  baseName: string,
  environments: Array<Pick<WorkspaceEnvironment, "id" | "name">>,
) {
  const base = baseName.trim() || "New Environment";
  if (!findDuplicateEnvironmentName(environments, base)) return base;

  let suffix = 2;
  while (findDuplicateEnvironmentName(environments, `${base} ${suffix}`)) {
    suffix += 1;
  }
  return `${base} ${suffix}`;
}

export function formatError(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

function normalizeEnvironmentName(name: string) {
  return name.trim().toLowerCase();
}
