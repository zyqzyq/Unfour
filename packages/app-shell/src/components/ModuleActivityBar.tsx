import { ListTree, Database, Globe2, Search, TerminalSquare } from "lucide-react";
import { ActivityBar, cn, useI18n } from "@unfour/ui";
import {
  getModuleSwitcherItems,
  type ModuleSwitcherItem,
} from "./module-helpers";

export function ModuleActivityBar({
  activeKind,
  onOpenCommandPalette,
  onPreload,
  sidebarCollapsed,
  onSelect,
  onToggleSidebar,
}: {
  activeKind: ModuleSwitcherItem["kind"] | null;
  onOpenCommandPalette: () => void;
  onPreload?: (kind: ModuleSwitcherItem["kind"]) => void;
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
    }
  }

  return (
    <ActivityBar>
      <div className="flex w-full flex-1 flex-col items-center gap-2">
        <nav aria-label={t("app.sidebar.modules")} className="flex w-full flex-col items-center gap-2">
          {getModuleSwitcherItems(t).map((item) => {
            const active = item.kind === activeKind;
            return (
              <button
                aria-current={active ? "page" : undefined}
                aria-expanded={active ? !sidebarCollapsed : undefined}
                aria-label={item.label}
                className={cn(
                  "u-module-button relative flex min-h-[58px] w-full flex-col items-center justify-center gap-1.5 rounded-[var(--u-radius-md)] text-[var(--u-color-text-muted)] transition-colors duration-150 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]",
                  active &&
                    "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)] shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--u-color-primary)_20%,transparent)]",
                )}
                key={item.id}
                onClick={() => handleClick(item)}
                onFocus={() => onPreload?.(item.kind)}
                onPointerDown={() => onPreload?.(item.kind)}
                onPointerEnter={() => onPreload?.(item.kind)}
                title={item.label}
                type="button"
              >
                {active && (
                  <span className="absolute inset-y-4 left-[-8px] w-[3px] rounded-r-[3px] bg-[var(--u-color-primary)]" />
                )}
                <ModuleIcon kind={item.kind} />
                <span className="max-w-full truncate text-[10px] font-semibold tracking-wide">{item.shortLabel}</span>
              </button>
            );
          })}
        </nav>
        <div className="flex-1" />
        <button
          aria-label={t("app.commandPalette.open")}
          className="flex h-9 w-9 items-center justify-center rounded-[var(--u-radius-md)] text-[var(--u-color-text-soft)] transition-colors duration-150 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]"
          onClick={onOpenCommandPalette}
          title={`${t("app.commandPalette.open")} (Ctrl/Cmd+Shift+P)`}
          type="button"
        >
          <Search size={18} />
        </button>
      </div>
    </ActivityBar>
  );
}

function ModuleIcon({ kind }: { kind: ModuleSwitcherItem["kind"] }) {
  if (kind === "flow") return <ListTree size={19} />;
  if (kind === "api") return <Globe2 size={19} />;
  if (kind === "ssh") return <TerminalSquare size={19} />;
  return <Database size={19} />;
}
