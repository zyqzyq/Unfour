import type {
  WorkspaceSidebarWidths,
  WorkspaceTab,
} from "@unfour/command-client";

export type ModuleSidebarKind = WorkspaceTab["kind"];

export type ModuleSidebarConfig = {
  defaultWidth: number;
  minWidth: number;
  maxWidth: number;
};

export const MODULE_SIDEBAR_CONFIG = {
  api: {
    defaultWidth: 320,
    minWidth: 220,
    maxWidth: 560,
  },
  ssh: {
    defaultWidth: 248,
    minWidth: 220,
    maxWidth: 420,
  },
  database: {
    defaultWidth: 280,
    minWidth: 220,
    maxWidth: 520,
  },
} as const satisfies Record<ModuleSidebarKind, ModuleSidebarConfig>;

export const DEFAULT_SIDEBAR_WIDTHS: WorkspaceSidebarWidths = {
  api: MODULE_SIDEBAR_CONFIG.api.defaultWidth,
  ssh: MODULE_SIDEBAR_CONFIG.ssh.defaultWidth,
  database: MODULE_SIDEBAR_CONFIG.database.defaultWidth,
};

export function normalizeModuleSidebarWidth(
  kind: ModuleSidebarKind,
  value: unknown,
) {
  const config = MODULE_SIDEBAR_CONFIG[kind];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return config.defaultWidth;
  }

  return Math.min(Math.max(value, config.minWidth), config.maxWidth);
}

export function normalizeSidebarWidths(
  widths: unknown,
  legacyWidth?: unknown,
): WorkspaceSidebarWidths {
  const widthRecord = isRecord(widths) ? widths : undefined;
  const source = widthRecord ? undefined : legacyWidth;

  return {
    api: normalizeModuleSidebarWidth(
      "api",
      widthRecord ? widthRecord.api : source,
    ),
    ssh: normalizeModuleSidebarWidth(
      "ssh",
      widthRecord ? widthRecord.ssh : source,
    ),
    database: normalizeModuleSidebarWidth(
      "database",
      widthRecord ? widthRecord.database : source,
    ),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
