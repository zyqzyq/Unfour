import type { DesktopAppExtensionContext, DesktopAppSettingsSection } from "@unfour/app-shell";
import { Button, ErrorState, StatusBadge, useFeedback, useI18n } from "@unfour/ui";
import { getAccountPrimaryLabel } from "../account/accountDisplay";
import { useAccount } from "../account/useAccount";
import { syncErrorMessageKey } from "./syncViewModel";
import { useCloudSync } from "./useCloudSync";

export function CloudSyncSection({ activeWorkspace }: DesktopAppExtensionContext) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const account = useAccount();
  const {
    available,
    hasCloudSyncCapability,
    errorCode,
    globalEnabled,
    loading,
    openCloudWorkspaceDialog,
    setServiceEnabled,
    statuses,
  } = useCloudSync();

  if (!available && errorCode) return <div className="flex flex-col gap-2">
    <StatusBadge tone="warning">{t("cloudSync.contextUnavailable")}</StatusBadge>
    <ErrorState>{t(syncErrorMessageKey(errorCode))}</ErrorState>
    <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.contextUnavailableDescription")}</p>
  </div>;

  if (account.state.kind !== "signedIn" || !hasCloudSyncCapability) return <div className="flex flex-col gap-2">
    <StatusBadge tone="warning">{t("cloudSync.capabilityRequired")}</StatusBadge>
    <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.capabilityDescription")}</p>
  </div>;

  if (!available) return <div className="flex flex-col gap-2">
    <StatusBadge tone="warning">{t("cloudSync.contextUnavailable")}</StatusBadge>
    <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.contextUnavailableDescription")}</p>
  </div>;

  const profile = account.state.profile;
  const copyDiagnostics = () => {
    const payload = {
      capturedAt: new Date().toISOString(),
      globalEnabled,
      activeWorkspaceId: activeWorkspace?.id ?? null,
      workspaces: [...statuses.entries()].map(([workspaceId, status]) => ({ workspaceId, ...status })),
    };
    void navigator.clipboard.writeText(JSON.stringify(payload, null, 2));
    feedback.success(t("cloudSync.diagnosticsCopied"));
  };

  return <div className="flex flex-col gap-4">
    {errorCode && <ErrorState>{t(syncErrorMessageKey(errorCode))}</ErrorState>}
    <section className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-3">
      <h3 className="text-xs font-semibold text-[var(--u-color-text-muted)]">{t("cloudSync.account")}</h3>
      <p className="mt-1 truncate text-sm font-semibold">{getAccountPrimaryLabel(profile, t("cloudSync.account"))}</p>
      <p className="truncate text-xs text-[var(--u-color-text-muted)]">{profile.email}</p>
    </section>

    <section className="border-b border-[var(--u-color-border)] pb-4">
      <div className="flex items-center justify-between gap-3">
        <div><h3 className="text-sm font-semibold">{t("cloudSync.syncService")}</h3><p className="mt-1 text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.syncServiceDescription")}</p></div>
        <input aria-label={t("cloudSync.syncService")} checked={globalEnabled} disabled={loading} onChange={(event) => void setServiceEnabled(event.target.checked).catch(() => undefined)} role="switch" type="checkbox" />
      </div>
      {!globalEnabled && <p className="mt-2 text-xs text-[var(--u-color-warning)]">{t("cloudSync.servicePausedDescription")}</p>}
    </section>

    <section className="grid grid-cols-2 gap-4 border-b border-[var(--u-color-border)] pb-4">
      <div><h3 className="text-sm font-semibold">{t("cloudSync.syncScope")}</h3><ul className="mt-2 space-y-1 text-xs"><li>✓ {t("cloudSync.scope.workspace")}</li><li>✓ {t("cloudSync.scope.connections")}</li><li>✓ {t("cloudSync.scope.environments")}</li><li>✓ {t("cloudSync.scope.variables")}</li><li>✓ {t("cloudSync.scope.apiCollections")}</li><li>✓ {t("cloudSync.scope.apiFolders")}</li><li>✓ {t("cloudSync.scope.apiRequests")}</li><li>✓ {t("cloudSync.scope.sshTasks")}</li></ul></div>
      <div><h3 className="text-sm font-semibold">{t("cloudSync.notSynced")}</h3><ul className="mt-2 list-disc space-y-1 pl-4 text-xs text-[var(--u-color-text-muted)]"><li>{t("cloudSync.scope.secrets")}</li><li>{t("cloudSync.scope.ssh")}</li><li>{t("cloudSync.scope.database")}</li><li>{t("cloudSync.scope.historyRuntime")}</li></ul></div>
    </section>
    <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.secretPolicy")}</p>
    <div><Button onClick={openCloudWorkspaceDialog} size="sm" type="button" variant="outline">{t("cloudSync.openCloudWorkspace")}</Button></div>
    <details className="border-t border-[var(--u-color-border)] pt-3"><summary className="cursor-pointer text-sm font-semibold">{t("cloudSync.advanced")}</summary><Button className="mt-2" onClick={copyDiagnostics} size="sm" type="button" variant="outline">{t("cloudSync.copyDiagnostics")}</Button></details>
  </div>;
}

function Label() {
  const { t } = useI18n();
  return <>{t("cloudSync.title")}</>;
}

export const cloudSyncSection: DesktopAppSettingsSection = {
  id: "cloud-sync.settings",
  label: <Label />,
  component: CloudSyncSection,
};
