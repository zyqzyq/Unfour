import { useState } from "react";
import type { AccountProfile } from "./accountTypes";
import { getAccountAvatarUrl, getAccountInitial } from "./accountDisplay";

export function AccountAvatar({
  accountFallback,
  profile,
  size = "titleBar",
}: {
  accountFallback: string;
  profile: AccountProfile;
  size?: "detail" | "titleBar";
}) {
  const avatarUrl = getAccountAvatarUrl(profile);
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const sizeClass = size === "detail" ? "h-10 w-10 text-sm" : "h-5 w-5 text-[10px]";
  const className = `${sizeClass} shrink-0 rounded-full border border-[var(--u-color-border)] object-cover`;

  if (avatarUrl && failedUrl !== avatarUrl) {
    return (
      <img
        alt=""
        className={className}
        onError={() => setFailedUrl(avatarUrl)}
        referrerPolicy="no-referrer"
        src={avatarUrl}
      />
    );
  }

  return (
    <span
      aria-hidden="true"
      className={`${className} inline-flex items-center justify-center bg-[var(--u-color-surface-muted)] font-semibold text-[var(--u-color-text-muted)]`}
    >
      {getAccountInitial(profile, accountFallback)}
    </span>
  );
}
