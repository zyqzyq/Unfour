import type { WorkspaceTab } from "@unfour/command-client";

type Translate = (key: string, fallback?: string) => string;

export type ModuleSwitcherItem = {
  id: "api-main" | "ssh-main" | "database-main" | "flow-main";
  kind: WorkspaceTab["kind"];
  label: string;
  shortLabel: string;
};

export function getModuleSwitcherItems(t?: Translate): ModuleSwitcherItem[] {
  return [
    {
      id: "api-main",
      kind: "api",
      label: translate(t, "app.nav.apiClient", "API Client"),
      shortLabel: translate(t, "app.nav.apiClientShort", "API"),
    },
    {
      id: "ssh-main",
      kind: "ssh",
      label: translate(t, "app.nav.sshTerminal", "SSH Terminal"),
      shortLabel: translate(t, "app.nav.sshTerminalShort", "SSH"),
    },
    {
      id: "database-main",
      kind: "database",
      label: translate(t, "app.nav.database", "Database"),
      shortLabel: translate(t, "app.nav.databaseShort", "DB"),
    },
    { id: "flow-main", kind: "flow", label: translate(t, "flow.title", "Flow"), shortLabel: translate(t, "flow.title", "Flow") },
  ];
}

export function moduleLabel(tab: WorkspaceTab, t?: Translate) {
  if (tab.kind === "flow") return translate(t, "flow.title", "Flow");
  if (tab.kind === "api") {
    return translate(t, "app.nav.apiClientShort", "API");
  }
  if (tab.kind === "ssh") {
    return translate(t, "app.nav.sshTerminalShort", "SSH");
  }
  return translate(t, "app.nav.database", "Database");
}

export function moduleSubtitle(tab: WorkspaceTab, t?: Translate) {
  if (tab.kind === "api") {
    return translate(t, "app.nav.apiClientSubtitle", "Collections and requests");
  }
  if (tab.kind === "ssh") {
    return translate(t, "app.nav.sshTerminalSubtitle", "Connections and sessions");
  }
  return translate(t, "app.nav.databaseSubtitle", "Connections and schemas");
}

export function isTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function translate(t: Translate | undefined, key: string, fallback: string) {
  return t ? t(key, fallback) : fallback;
}
