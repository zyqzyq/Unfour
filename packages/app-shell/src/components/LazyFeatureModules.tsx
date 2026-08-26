import { LoadingState } from "@unfour/ui";
import { lazy, Suspense, type ComponentProps, type ReactNode } from "react";
import {
  loadApiClientModule,
  loadDatabaseModule,
  loadSshTerminalModule,
  loadWorkspaceEnvironmentsModule,
} from "./featureModuleLoaders";

const LazyApiClientPage = lazy(() =>
  loadApiClientModule().then((module) => ({ default: module.ApiClientPage })),
);

const LazyDatabasePage = lazy(() =>
  loadDatabaseModule().then((module) => ({ default: module.DatabasePage })),
);

const LazyTerminalLogPanel = lazy(() =>
  loadSshTerminalModule().then((module) => ({ default: module.TerminalLogPanel })),
);

const LazyTerminalPage = lazy(() =>
  loadSshTerminalModule().then((module) => ({ default: module.TerminalPage })),
);

const LazyTerminalStatusBar = lazy(() =>
  loadSshTerminalModule().then((module) => ({ default: module.TerminalStatusBar })),
);

const LazyWorkspaceEnvironmentsPage = lazy(() =>
  loadWorkspaceEnvironmentsModule().then((module) => ({
    default: module.WorkspaceEnvironmentsPage,
  })),
);

const LazyWorkspaceEnvironmentsStatusBar = lazy(() =>
  loadWorkspaceEnvironmentsModule().then((module) => ({
    default: module.WorkspaceEnvironmentsStatusBar,
  })),
);

function FeatureModuleLoadingState() {
  return <LoadingState className="h-full min-h-0 rounded-none border-0" />;
}

export function ApiClientModule(props: ComponentProps<typeof LazyApiClientPage>) {
  return (
    <Suspense fallback={<FeatureModuleLoadingState />}>
      <LazyApiClientPage {...props} />
    </Suspense>
  );
}

export function DatabaseModule(props: ComponentProps<typeof LazyDatabasePage>) {
  return (
    <Suspense fallback={<FeatureModuleLoadingState />}>
      <LazyDatabasePage {...props} />
    </Suspense>
  );
}

export function SshTerminalModule(props: ComponentProps<typeof LazyTerminalPage>) {
  return (
    <Suspense fallback={<FeatureModuleLoadingState />}>
      <LazyTerminalPage {...props} />
    </Suspense>
  );
}

export function SshTerminalLogPanel({
  fallback,
  ...props
}: ComponentProps<typeof LazyTerminalLogPanel> & { fallback: ReactNode }) {
  return (
    <Suspense fallback={fallback}>
      <LazyTerminalLogPanel {...props} />
    </Suspense>
  );
}

export function SshTerminalStatusBar({
  fallback,
  ...props
}: ComponentProps<typeof LazyTerminalStatusBar> & { fallback: ReactNode }) {
  return (
    <Suspense fallback={fallback}>
      <LazyTerminalStatusBar {...props} />
    </Suspense>
  );
}

export function WorkspaceEnvironmentsModule(
  props: ComponentProps<typeof LazyWorkspaceEnvironmentsPage>,
) {
  return (
    <Suspense fallback={<FeatureModuleLoadingState />}>
      <LazyWorkspaceEnvironmentsPage {...props} />
    </Suspense>
  );
}

export function WorkspaceEnvironmentsModuleStatusBar({
  fallback,
  ...props
}: ComponentProps<typeof LazyWorkspaceEnvironmentsStatusBar> & {
  fallback: ReactNode;
}) {
  return (
    <Suspense fallback={fallback}>
      <LazyWorkspaceEnvironmentsStatusBar {...props} />
    </Suspense>
  );
}
