import { Check, ChevronDown, Settings2 } from "lucide-react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Button } from "./button";
import { cn } from "./utils";
import { useI18n } from "./i18n";

export type EnvironmentSelectItem = {
  id: string;
  name: string;
};

export function ActiveEnvironmentSelect({
  activeEnvironmentId,
  environments,
  onManage,
  onSelect,
}: {
  activeEnvironmentId: string | null;
  environments: EnvironmentSelectItem[];
  onManage: () => void;
  onSelect: (environmentId: string | null) => void;
}) {
  const { t } = useI18n();
  const activeEnvironment = environments.find(
    (environment) => environment.id === activeEnvironmentId,
  );

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          aria-label={t("variables.activeEnvironment")}
          className="max-w-[132px] justify-start gap-1.5 border-transparent bg-[var(--u-color-surface)] px-1.5 font-medium shadow-none hover:bg-[var(--u-color-surface-hover)]"
          size="sm"
          title={t("variables.environmentPrefix", {
            name: activeEnvironment?.name ?? t("variables.noEnvironment"),
          })}
          type="button"
          variant="outline"
        >
          <span
            className={cn(
              "h-1.5 w-1.5 shrink-0 rounded-full",
              activeEnvironment
                ? "bg-[var(--u-color-primary)]"
                : "bg-[var(--u-color-text-soft)]",
            )}
          />
          <span className="min-w-0 truncate">
            {activeEnvironment?.name ?? t("variables.noEnvironment")}
          </span>
          <ChevronDown className="shrink-0 text-[var(--u-color-text-muted)]" size={12} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="start"
          className="z-50 min-w-56 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1 text-[12px] text-[var(--u-color-text)] shadow-xl"
          sideOffset={6}
        >
          <EnvironmentItem
            active={activeEnvironmentId === null}
            label={t("variables.noEnvironment")}
            onSelect={() => onSelect(null)}
          />
          {environments.length > 0 && (
            <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
          )}
          {environments.map((environment) => (
            <EnvironmentItem
              active={activeEnvironmentId === environment.id}
              key={environment.id}
              label={environment.name}
              onSelect={() => onSelect(environment.id)}
            />
          ))}
          <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
          <DropdownMenu.Item
            className="flex h-8 cursor-pointer items-center gap-2 rounded-[var(--u-radius-sm)] px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)]"
            onSelect={onManage}
          >
            <Settings2 size={13} />
            {t("variables.manage")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function EnvironmentItem({
  active,
  label,
  onSelect,
}: {
  active: boolean;
  label: string;
  onSelect: () => void;
}) {
  return (
    <DropdownMenu.Item
      className={cn(
        "flex h-8 cursor-pointer items-center gap-2 rounded-[var(--u-radius-sm)] px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)]",
        active && "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]",
      )}
      onSelect={onSelect}
    >
      <Check className={active ? "text-current" : "text-transparent"} size={13} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
    </DropdownMenu.Item>
  );
}
