import type { TFunction } from "@unfour/ui";

export function redisErrorMessage(error: unknown, t: TFunction): string {
  const code = typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
  const fallback = t("redis.errors.unknown");
  return code ? t(`redis.errors.${code}`, `${fallback} (${code})`) : fallback;
}
