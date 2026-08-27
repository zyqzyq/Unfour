import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  useI18n,
} from "@unfour/ui";
import { AccountAvatar } from "./AccountAvatar";
import {
  getAccountEmail,
  getAccountPrimaryLabel,
  getAccountUsername,
  getCompactAccountLabel,
} from "./accountDisplay";
import type { AccountProfile } from "./accountTypes";
import { useAccount } from "./useAccount";

function SignedInIndicator({ profile }: { profile: AccountProfile }) {
  const { t } = useI18n();
  const { openOverlay, signOut } = useAccount();
  const accountFallback = t("account.title");
  const compactLabel = getCompactAccountLabel(profile, accountFallback);
  const primaryLabel = getAccountPrimaryLabel(profile, accountFallback);
  const username = getAccountUsername(profile);
  const email = getAccountEmail(profile);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label={t("account.signedInAs", { account: compactLabel })}
          className="h-7 w-7 px-0"
          size="sm"
          title={compactLabel}
          type="button"
          variant="ghost"
        >
          <AccountAvatar accountFallback={accountFallback} profile={profile} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="w-64 max-w-[calc(100vw-16px)]"
        collisionPadding={8}
      >
        <div className="border-b border-[var(--u-color-border)] px-2 py-2">
          <p className="truncate text-[13px] font-semibold" title={primaryLabel}>{primaryLabel}</p>
          {username && (
            <p className="mt-0.5 truncate text-[var(--u-color-text-muted)]" title={`@${username}`}>
              @{username}
            </p>
          )}
          {email && (
            <p className="mt-0.5 break-all text-[var(--u-color-text-soft)]" title={email}>{email}</p>
          )}
        </div>
        <DropdownMenuItem onSelect={openOverlay}>{t("account.accountDetails")}</DropdownMenuItem>
        <DropdownMenuItem
          className="text-[var(--u-color-danger)] data-[highlighted]:bg-[var(--u-color-danger-soft)]"
          onSelect={signOut}
        >
          {t("account.signOut")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function AccountIndicator() {
  const { t } = useI18n();
  const { openOverlay, state } = useAccount();
  if (state.kind === "signedIn") return <SignedInIndicator profile={state.profile} />;

  const label = state.kind === "signingIn"
    ? t("account.indicatorSigningIn")
    : state.kind === "error"
      ? t("account.indicatorError")
      : t("account.indicatorSignedOut");

  return (
    <Button
      className={`h-7 max-w-32 truncate px-2 ${state.kind === "error" ? "text-[var(--u-color-danger)]" : ""}`}
      onClick={openOverlay}
      size="sm"
      title={label}
      type="button"
      variant="ghost"
    >
      {label}
    </Button>
  );
}
