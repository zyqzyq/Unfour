import { PanelLeftOpen } from "lucide-react";
import { IconButton, RightInspector } from "@unfour/ui";
import type { WorkspaceTab } from "@unfour/command-client";
import { moduleLabel } from "./module-helpers";

export function RightInspectorPlaceholder({
  activeTab,
  collapsed,
  onCollapse,
  onWidthChange,
  width,
}: {
  activeTab: WorkspaceTab;
  collapsed: boolean;
  onCollapse: () => void;
  onWidthChange: (width: number) => void;
  width: number;
}) {
  return (
    <RightInspector
      collapsed={collapsed}
      onWidthChange={onWidthChange}
      resizable
      width={width}
    >
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <div className="text-[12px] font-semibold text-[var(--u-color-text)]">
          Inspector
        </div>
        <IconButton label="Collapse inspector" onClick={onCollapse}>
          <PanelLeftOpen size={14} />
        </IconButton>
      </div>
      <div className="p-2 text-[12px] text-[var(--u-color-text-muted)]">
        {moduleLabel(activeTab)} details and properties will use this space.
      </div>
    </RightInspector>
  );
}
