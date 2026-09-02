import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import { useI18n } from "@unfour/ui";
import { AccountSection } from "./features/account";
import { CloudSyncSection } from "./features/cloud-sync";

export function AccountSyncSettings(context: DesktopAppExtensionContext) {
  const { t } = useI18n();

  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.accountSync.title")}
        </h2>
        <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {t("app.settings.accountSync.description")}
        </p>
      </div>

      <section className="space-y-3">
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.accountSync.accountTitle")}
        </h3>
        <AccountSection />
      </section>

      <section className="space-y-3 border-t border-[var(--u-color-border)] pt-4">
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.accountSync.cloudSyncTitle")}
        </h3>
        <CloudSyncSection {...context} />
      </section>
    </div>
  );
}
