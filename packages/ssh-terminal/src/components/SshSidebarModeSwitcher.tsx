import { useI18n } from "@unfour/ui";

export type SshSidebarMode = "connections" | "tasks";

export function SshSidebarModeSwitcher({
  activeMode,
  onChange,
}: {
  activeMode: SshSidebarMode;
  onChange: (mode: SshSidebarMode) => void;
}) {
  const { t } = useI18n();
  const modes: SshSidebarMode[] = ["connections", "tasks"];

  return (
    <div
      aria-label={t("ssh.title")}
      className="flex min-w-0 flex-1 items-center gap-0.5"
      role="tablist"
    >
      {modes.map((mode) => {
        const active = mode === activeMode;
        return (
          <button
            aria-selected={active}
            className={`h-6 cursor-pointer rounded-[var(--u-radius-sm)] px-2 text-[11px] font-semibold transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)] ${
              active
                ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
            }`}
            key={mode}
            onClick={() => onChange(mode)}
            role="tab"
            type="button"
          >
            {t(`ssh.homeTabs.${mode}`)}
          </button>
        );
      })}
    </div>
  );
}
