import type { ReactNode } from "react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  EmptyState,
  ErrorState,
  LoadingState,
  useI18n,
} from "@unfour/ui";
import { AccountAvatar } from "./AccountAvatar";
import {
  getAccountEmail,
  getAccountPrimaryLabel,
  getAccountUsername,
} from "./accountDisplay";
import { AccountPlanSummary } from "./AccountPlanSummary";
import { useAccount } from "./useAccount";

function Row({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="flex justify-between gap-4 py-1">
      <span className="shrink-0 text-[var(--u-color-text-muted)]">{label}</span>
      <span className="min-w-0 break-words text-right font-medium">{value}</span>
    </div>
  );
}

export function AccountOverlays() {
  const { t } = useI18n();
  const { overlayOpen, retry, setOverlayOpen, signIn, signOut, state } = useAccount();

  const accountFallback = t("account.title");
  const signedInProfile = state.kind === "signedIn" ? state.profile : null;
  const primaryLabel = signedInProfile
    ? getAccountPrimaryLabel(signedInProfile, accountFallback)
    : accountFallback;
  const username = signedInProfile ? getAccountUsername(signedInProfile) : null;
  const email = signedInProfile ? getAccountEmail(signedInProfile) : null;
  const hasDetailRows = Boolean(username || email);

  return (
    <Dialog onOpenChange={setOverlayOpen} open={overlayOpen}>
      <DialogContent title={t("account.title")}>
        <DialogHeader>
          <DialogTitle>{t("account.title")}</DialogTitle>
          <DialogXClose label={t("account.close")} />
        </DialogHeader>
        <DialogBody>
          {state.kind === "signedOut" && <EmptyState>{t("account.signedOutDescription")}</EmptyState>}
          {state.kind === "signingIn" && <LoadingState>{t("account.signingInDescription")}</LoadingState>}
          {state.kind === "error" && <ErrorState>{t("account.errorDescription")}</ErrorState>}
          {signedInProfile && (
            <div className="flex flex-col gap-3">
              <div className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-3">
              <div className="flex min-w-0 items-center gap-3">
                <AccountAvatar
                  accountFallback={accountFallback}
                  profile={signedInProfile}
                  size="detail"
                />
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold" title={primaryLabel}>{primaryLabel}</p>
                  {username && <p className="truncate text-xs text-[var(--u-color-text-muted)]" title={`@${username}`}>@{username}</p>}
                  {email && <p className="break-all text-xs text-[var(--u-color-text-soft)]" title={email}>{email}</p>}
                </div>
              </div>
              {hasDetailRows && (
                <div className="mt-3 border-t border-[var(--u-color-border)] pt-2">
                  {username && <Row label={t("account.username")} value={username} />}
                  {email && <Row label={t("account.email")} value={email} />}
                </div>
              )}
              <p className="mt-2 border-t border-[var(--u-color-border)] pt-2 text-xs text-[var(--u-color-text-muted)]">
                {t("account.signedInDescription")}
              </p>
              </div>
              <AccountPlanSummary profile={signedInProfile} />
            </div>
          )}
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => setOverlayOpen(false)} size="sm" type="button" variant="ghost">{t("account.close")}</Button>
          {state.kind === "signedOut" && <Button onClick={signIn} size="sm" type="button">{t("account.signIn")}</Button>}
          {state.kind === "signingIn" && <Button disabled size="sm" type="button">{t("account.signingIn")}</Button>}
          {state.kind === "error" && <Button onClick={retry} size="sm" type="button">{t("account.retry")}</Button>}
          {state.kind === "signedIn" && <Button onClick={signOut} size="sm" type="button" variant="outline">{t("account.signOut")}</Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
