import {
  Database,
  Globe2,
  TerminalSquare,
} from "lucide-react";
import { SidebarRow, cn } from "@unfour/ui";
import {
  getModuleSwitcherItems,
  type ModuleSwitcherItem,
} from "./module-helpers";

export function ModuleSwitcher({
  activeKind,
  collapsed,
  onSelect,
}: {
  activeKind: ModuleSwitcherItem["kind"];
  collapsed: boolean;
  onSelect: (tabId: ModuleSwitcherItem["id"]) => void;
}) {
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
      <div
        className={cn(
          collapsed
            ? "space-y-1"
            : "grid grid-cols-3 gap-1",
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
