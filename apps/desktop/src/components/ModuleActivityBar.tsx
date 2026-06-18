import { Database, Globe2, TerminalSquare } from "lucide-react";
import { ActivityBar, cn, useI18n } from "@unfour/ui";
import {
  getModuleSwitcherItems,
  type ModuleSwitcherItem,
} from "./module-helpers";

export function ModuleActivityBar({
  activeKind,
  sidebarCollapsed,
  onSelect,
  onToggleSidebar,
}: {
  activeKind: ModuleSwitcherItem["kind"];
  sidebarCollapsed: boolean;
  onSelect: (tabId: ModuleSwitcherItem["id"]) => void;
  onToggleSidebar: () => void;
}) {
  const { t } = useI18n();

  function handleClick(item: ModuleSwitcherItem) {
    if (item.kind === activeKind) {
      onToggleSidebar();
    } else {
      onSelect(item.id);
      if (sidebarCollapsed) {
        onToggleSidebar();
      }
    }
  }

  return (
    <ActivityBar>
      <nav aria-label={t("app.sidebar.modules")} className="flex w-full flex-col items-center gap-1">
        {getModuleSwitcherItems(t).map((item) => (
          <button
            aria-label={item.label}
            aria-pressed={item.kind === activeKind && !sidebarCollapsed}
            className={cn(
              "flex h-9 w-9 items-center justify-center rounded-[var(--u-radius-md)] text-[var(--u-color-text-muted)] transition-colors duration-150 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]",
              item.kind === activeKind &&
                !sidebarCollapsed &&
                "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)] ring-1 ring-inset ring-[var(--u-color-border)]",
            )}
            key={item.id}
            onClick={() => handleClick(item)}
            title={item.label}
            type="button"
          >
            <ModuleIcon kind={item.kind} />
          </button>
        ))}
      </nav>
    </ActivityBar>
  );
}

function ModuleIcon({ kind }: { kind: ModuleSwitcherItem["kind"] }) {
  if (kind === "api") return <Globe2 size={16} />;
  if (kind === "ssh") return <TerminalSquare size={16} />;
  return <Database size={16} />;
}
