import type { QueryClient } from "@tanstack/react-query";
import type { WorkspaceTab } from "@unfour/command-client";
import { useCallback, useEffect } from "react";
import {
  preloadFeatureModule,
  type FeatureModulePreloadContext,
} from "./featureModuleLoaders";

const FEATURE_PRELOAD_ORDER: WorkspaceTab["kind"][] = ["api", "database", "ssh"];

type IdleWindow = Window &
  typeof globalThis & {
    cancelIdleCallback?: (handle: number) => void;
    requestIdleCallback?: (
      callback: () => void,
      options?: { timeout?: number },
    ) => number;
  };

export function useFeatureModulePreload(
  activeKind: WorkspaceTab["kind"],
  options?: {
    preload?: (
      kind: WorkspaceTab["kind"],
      context?: FeatureModulePreloadContext,
    ) => Promise<unknown>;
    queryClient?: QueryClient;
    workspaceId?: string;
  },
) {
  const preload = options?.preload ?? preloadFeatureModule;
  const queryClient = options?.queryClient;
  const workspaceId = options?.workspaceId;
  const preloadKind = useCallback(
    (kind: WorkspaceTab["kind"]) =>
      preload(
        kind,
        queryClient && workspaceId ? { queryClient, workspaceId } : undefined,
      ).catch(() => undefined),
    [preload, queryClient, workspaceId],
  );

  useEffect(() => {
    if (typeof window === "undefined") return;

    const pendingKinds = FEATURE_PRELOAD_ORDER.filter((kind) => kind !== activeKind);
    const idleWindow = window as IdleWindow;
    let cancelled = false;
    let cancelScheduled: (() => void) | null = null;

    const scheduleNext = () => {
      if (cancelled) return;
      const nextKind = pendingKinds.shift();
      if (!nextKind) return;

      cancelScheduled = scheduleIdle(idleWindow, () => {
        cancelScheduled = null;
        if (cancelled) return;
        void preloadKind(nextKind)
          // Preloading is best-effort. A rejected import is reset by the cached
          // loader so the visible Suspense boundary can retry on real navigation.
          .catch(() => undefined)
          .finally(scheduleNext);
      });
    };

    scheduleNext();
    return () => {
      cancelled = true;
      cancelScheduled?.();
    };
  }, [activeKind, preloadKind]);

  return preloadKind;
}

function scheduleIdle(idleWindow: IdleWindow, callback: () => void) {
  if (idleWindow.requestIdleCallback) {
    const handle = idleWindow.requestIdleCallback(callback, { timeout: 2_000 });
    return () => idleWindow.cancelIdleCallback?.(handle);
  }

  const handle = idleWindow.setTimeout(callback, 800);
  return () => idleWindow.clearTimeout(handle);
}
