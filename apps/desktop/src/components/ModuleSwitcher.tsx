import {
  Database,
  Globe2,
  PanelLeftClose,
  PanelLeftOpen,
  TerminalSquare,
} from "lucide-react";
import { IconButton, SidebarRow, cn } from "@unfour/ui";
import {
  getModuleSwitcherItems,
  type ModuleSwitcherItem,
} from "./module-helpers";

export function ModuleSwitcher({
  activeKind,
  collapsed,
  onToggle,
  onSelect,
}: {
  activeKind: ModuleSwitcherItem["kind"];
  collapsed: boolean;
  onToggle: () => void;
  onSelect: (tabId: ModuleSwitcherItem["id"]) => void;
}) {
  const activeItem =
    getModuleSwitcherItems().find((item) => item.kind === activeKind) ??
    getModuleSwitcherItems()[0];

  return (
    <nav
      aria-label="Modules"
      className={cn(
        "w-full",
        collapsed
          ? "space-y-1"
          : "rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1",
      )}
    >
      {!collapsed && (
        <div className="flex items-center gap-2 px-1.5 py-1">
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[var(--u-radius-sm)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]">
            <ModuleIcon kind={activeItem.kind} />
          </span>
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[var(--u-color-text)]">
            {activeItem.label}
          </span>
          <IconButton label="Toggle sidebar" onClick={onToggle}>
            <PanelLeftClose size={15} />
          </IconButton>
        </div>
      )}
      <div
        className={cn(
          collapsed
            ? "space-y-1"
            : "grid grid-cols-3 gap-1 border-t border-[var(--u-color-border)] pt-1",
        )}
      >
        {getModuleSwitcherItems().map((item) => (
          <SidebarRow
            active={item.kind === activeKind}
            className={cn(
              "justify-center",
              collapsed
                ? "px-0"
                : "h-7 gap-1.5 border border-transparent px-2 text-[11px] font-semibold",
            )}
            key={item.id}
            onClick={() => onSelect(item.id)}
            title={item.label}
          >
            <ModuleIcon kind={item.kind} />
            {!collapsed && <span className="truncate">{item.shortLabel}</span>}
          </SidebarRow>
        ))}
      </div>
      {collapsed && (
        <IconButton className="w-full" label="Toggle sidebar" onClick={onToggle}>
          <PanelLeftOpen size={15} />
        </IconButton>
      )}
    </nav>
  );
}

function ModuleIcon({ kind }: { kind: ModuleSwitcherItem["kind"] }) {
  if (kind === "api") {
    return <Globe2 size={14} />;
  }
  if (kind === "ssh") {
    return <TerminalSquare size={14} />;
  }
  return <Database size={14} />;
}
