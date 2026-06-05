import * as React from "react";
import { AppShellFrame } from "@unfour/ui";

export type AppShellProps = {
  bottomPanel?: React.ReactNode;
  className?: string;
  globalToolbar?: React.ReactNode;
  main: React.ReactNode;
  rightInspector?: React.ReactNode;
  sidebar?: React.ReactNode;
  statusBar?: React.ReactNode;
};

export default function AppShell({
  bottomPanel,
  className,
  globalToolbar,
  main,
  rightInspector,
  sidebar,
  statusBar,
}: AppShellProps) {
  return (
    <AppShellFrame
      bottomPanel={bottomPanel}
      className={className}
      globalToolbar={globalToolbar ?? null}
      rightInspector={rightInspector}
      sidebar={sidebar ?? null}
      statusBar={statusBar}
    >
      {main}
    </AppShellFrame>
  );
}
