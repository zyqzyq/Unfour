import { Button, Select, StatusBadge, useI18n } from "@unfour/ui";
import {
  getAccountEmail,
  getAccountPrimaryLabel,
  getAccountUsername,
} from "./accountDisplay";
import type { AccountMockState } from "./accountTypes";
import { AccountPlanSummary } from "./AccountPlanSummary";
import { useAccount } from "./useAccount";

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return <div className="flex justify-between gap-4 py-1 text-sm"><span className="text-[var(--u-color-text-muted)]">{label}</span><span className="min-w-0 truncate font-medium">{value}</span></div>;
}

export function AccountSection() {
  const { t } = useI18n();
  const { preview, retry, setMockState, signIn, signOut, state } = useAccount();
  const status = state.kind === "signedIn"
    ? { label: t("account.statusSignedIn"), tone: "success" as const }
    : state.kind === "signingIn"
      ? { label: t("account.statusSigningIn"), tone: "warning" as const }
      : state.kind === "error"
        ? { label: t("account.statusError"), tone: "danger" as const }
        : { label: t("account.statusSignedOut"), tone: "neutral" as const };
  const mockOptions = [
    { label: t("account.mockSignedOut"), value: "signedOut" },
    { label: t("account.mockSigningIn"), value: "signingIn" },
    { label: t("account.mockSignedIn"), value: "signedIn" },
    { label: t("account.mockError"), value: "error" },
  ];

  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-2">
        <Row label={t("account.status")} value={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>} />
        {state.kind === "signedIn" && (() => {
          const accountFallback = t("account.title");
          const username = getAccountUsername(state.profile);
          const email = getAccountEmail(state.profile);
          return <>
            <Row label={t("account.displayName")} value={getAccountPrimaryLabel(state.profile, accountFallback)} />
            {username && <Row label={t("account.username")} value={username} />}
            {email && <Row label={t("account.email")} value={email} />}
          </>;
        })()}
      </div>

      {state.kind === "signedIn" && <AccountPlanSummary profile={state.profile} />}

      <div className="flex gap-2">
        {state.kind === "signedOut" && <Button onClick={signIn} size="sm" type="button">{t("account.signIn")}</Button>}
        {state.kind === "signingIn" && <Button disabled size="sm" type="button">{t("account.signingIn")}</Button>}
        {state.kind === "signedIn" && <Button onClick={signOut} size="sm" type="button" variant="outline">{t("account.signOut")}</Button>}
        {state.kind === "error" && <Button onClick={retry} size="sm" type="button">{t("account.retry")}</Button>}
      </div>

      {preview && <section className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] p-3">
        <h3 className="text-sm font-semibold">{t("account.testPreviewTitle")}</h3>
        <p className="mt-1 text-xs text-[var(--u-color-text-muted)]">{t("account.testChannelNotice")}</p>
        <label className="mt-3 block text-xs font-medium text-[var(--u-color-text-muted)]" htmlFor="account-mock-state">{t("account.mockState")}</label>
        <Select
          className="mt-1"
          id="account-mock-state"
          onChange={(event) => setMockState(event.target.value as AccountMockState)}
          options={mockOptions}
          value={state.kind}
        />
        <p className="mt-2 text-xs text-[var(--u-color-text-muted)]">{t("account.capabilityNotice")}</p>
      </section>}
    </div>
  );
}
