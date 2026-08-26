import type { WorkspaceTab } from "@unfour/command-client";
import { useCallback, useState } from "react";

type PersistentFeatureKind = WorkspaceTab["kind"];

function isPersistentFeatureKind(
  kind: WorkspaceTab["kind"],
): kind is PersistentFeatureKind {
  return kind === "api" || kind === "database" || kind === "ssh";
}

export function usePersistentFeatureMounts({
  activeTabId,
  setActiveTab: setActiveTabInStore,
  tabs,
}: {
  activeTabId: string;
  setActiveTab: (tabId: string) => void;
  tabs: readonly WorkspaceTab[];
}) {
  const activeKind = tabs.find((tab) => tab.id === activeTabId)?.kind;
  const [mountedFeatures, setMountedFeatures] = useState<
    ReadonlySet<PersistentFeatureKind>
  >(
    () =>
      new Set<PersistentFeatureKind>(
        activeKind && isPersistentFeatureKind(activeKind) ? [activeKind] : [],
      ),
  );

  const setActiveTab = useCallback(
    (tabId: string) => {
      const nextKind = tabs.find((tab) => tab.id === tabId)?.kind;
      setMountedFeatures((current) =>
        rememberPersistentFeatures(current, [activeKind, nextKind]),
      );
      setActiveTabInStore(tabId);
    },
    [activeKind, setActiveTabInStore, tabs],
  );

  return {
    setActiveTab,
    shouldMountApi: activeKind === "api" || mountedFeatures.has("api"),
    shouldMountDatabase:
      activeKind === "database" || mountedFeatures.has("database"),
    shouldMountSsh: activeKind === "ssh" || mountedFeatures.has("ssh"),
  };
}

function rememberPersistentFeatures(
  current: ReadonlySet<PersistentFeatureKind>,
  kinds: readonly (WorkspaceTab["kind"] | undefined)[],
) {
  const persistentKinds = kinds.filter(
    (kind): kind is PersistentFeatureKind =>
      kind !== undefined && isPersistentFeatureKind(kind),
  );
  if (persistentKinds.every((kind) => current.has(kind))) return current;
  return new Set<PersistentFeatureKind>([...current, ...persistentKinds]);
}
