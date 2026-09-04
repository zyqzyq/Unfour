import { useState } from "react";
import { Button, useI18n } from "@unfour/ui";
import {
  getAccountCommandErrorCode,
  openAccountUpgrade,
  openWebAccount,
} from "./accountApi";
import { formatAccountDate, getActiveCloudSyncEntitlement } from "./accountEntitlement";
import type { AccountProfile } from "./accountTypes";
import { useAccount } from "./useAccount";

type PlanAction = "account" | "refresh" | "upgrade";
type PlanActionError = { action: PlanAction; code: string | null } | null;

export function AccountPlanSummary({ profile }: { profile: AccountProfile }) {
  const { locale, t } = useI18n();
  const { refreshAccount, refreshing } = useAccount();
  const [actionError, setActionError] = useState<PlanActionError>(null);
  const [pendingAction, setPendingAction] = useState<PlanAction | null>(null);
  const entitlement = getActiveCloudSyncEntitlement(profile.entitlements);
  const activeUntil = formatAccountDate(entitlement?.validUntil, locale);

  const runAction = (
    action: PlanAction,
    operation: () => Promise<unknown>,
  ) => {
    setActionError(null);
    setPendingAction(action);
    void operation()
      .catch((error: unknown) => {
        setActionError({ action, code: getAccountCommandErrorCode(error) });
      })
      .finally(() => setPendingAction(null));
  };

  const billingErrorKey = actionError?.code === "signed_out"
    || actionError?.code === "unauthorized"
    || actionError?.code === "desktop_session_expired"
    ? "account.billingSignInRequired"
    : actionError?.code === "api_unavailable"
      || actionError?.code === "not_ready"
      || actionError?.code === "billing_unavailable"
      ? "account.billingApiUnavailable"
      : actionError?.code === "invalid_api_response"
        || actionError?.code === "invalid_billing_url"
        ? "account.billingInvalidResponse"
        : actionError?.code === "billing_already_active"
          ? "account.billingAlreadyActive"
          : actionError?.code === "billing_customer_not_found"
            ? "account.billingCustomerNotFound"
            : actionError?.code === "checkout_page_open_failed"
              ? "account.openUpgradeFailed"
              : actionError?.code === "billing_portal_open_failed"
                ? "account.openAccountFailed"
                : "account.billingRequestFailed";
  const errorMessage = actionError?.action === "refresh"
    ? t("account.refreshFailed")
    : actionError
      ? t(billingErrorKey)
      : null;

  return (
    <section className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-3">
      <h3 className="text-xs font-semibold text-[var(--u-color-text-muted)]">
        {t("account.plan")}
      </h3>
      <p className="mt-1 text-sm font-semibold">
        {entitlement ? t("account.proPlan") : t("account.freePlan")}
      </p>
      <p className="mt-0.5 text-xs text-[var(--u-color-text-muted)]">
        {entitlement
          ? activeUntil
            ? t("account.activeUntil", { date: activeUntil })
            : t("account.active")
          : t("account.cloudSyncRequiresPro")}
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        {entitlement ? (
          <Button
            disabled={pendingAction !== null}
            onClick={() => runAction("account", openWebAccount)}
            size="sm"
            type="button"
          >
            {t("account.accountAndBilling")}
          </Button>
        ) : (
          <Button
            disabled={pendingAction !== null}
            onClick={() => runAction("upgrade", openAccountUpgrade)}
            size="sm"
            type="button"
          >
            {t("account.upgradeToPro")}
          </Button>
        )}
        <Button
          disabled={refreshing || pendingAction !== null}
          onClick={() => runAction("refresh", refreshAccount)}
          size="sm"
          type="button"
          variant="outline"
        >
          {refreshing ? t("account.refreshing") : t("account.refreshAccount")}
        </Button>
      </div>
      {errorMessage && (
        <p className="mt-2 text-xs text-[var(--u-color-danger)]" role="alert">
          {errorMessage}
        </p>
      )}
    </section>
  );
}
