import type { AccountProfile } from "./accountTypes";

type AccountIdentity = Pick<
  AccountProfile,
  "avatarUrl" | "displayName" | "email" | "username"
>;

function compactValue(value: string | null | undefined): string | null {
  const compact = value?.trim();
  return compact ? compact : null;
}

function emailPrefix(email: string | null | undefined): string | null {
  const compactEmail = compactValue(email);
  if (!compactEmail) return null;
  return compactValue(compactEmail.split("@", 1)[0]);
}

function fallbackValue(fallback: string): string {
  return compactValue(fallback) ?? "Account";
}

export function getAccountUsername(profile: AccountIdentity): string | null {
  return compactValue(profile.username);
}

export function getAccountDisplayName(profile: AccountIdentity): string | null {
  return compactValue(profile.displayName);
}

export function getAccountEmail(profile: AccountIdentity): string | null {
  return compactValue(profile.email);
}

export function getAccountAvatarUrl(profile: AccountIdentity): string | null {
  return compactValue(profile.avatarUrl);
}

export function getCompactAccountLabel(
  profile: AccountIdentity,
  accountFallback = "Account",
): string {
  return getAccountUsername(profile)
    ?? getAccountDisplayName(profile)
    ?? emailPrefix(profile.email)
    ?? fallbackValue(accountFallback);
}

export function getAccountPrimaryLabel(
  profile: AccountIdentity,
  accountFallback = "Account",
): string {
  return getAccountDisplayName(profile)
    ?? getAccountUsername(profile)
    ?? emailPrefix(profile.email)
    ?? fallbackValue(accountFallback);
}

export function getAccountInitial(
  profile: AccountIdentity,
  accountFallback = "Account",
): string {
  const source = getAccountUsername(profile) ?? fallbackValue(accountFallback);
  return Array.from(source)[0]?.toLocaleUpperCase() ?? "A";
}
