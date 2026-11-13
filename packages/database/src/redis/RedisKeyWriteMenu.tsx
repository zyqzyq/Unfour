import { Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, useI18n } from "@unfour/ui";
import type { RedisKeyInfo } from "@unfour/command-client";
import type { WriteKind } from "./redis-write";

export function RedisKeyWriteMenu({ info, fieldBase64, select }: {
  info: RedisKeyInfo; fieldBase64?: string; select: (kind: WriteKind) => void;
}) {
  const { t } = useI18n();
  const complete = Boolean(info.preview && !info.preview.truncated);
  const entry = (kind: WriteKind, disabled = false, destructive = false) => <DropdownMenuItem disabled={disabled}
    className={destructive ? "text-[var(--u-color-danger)]" : undefined} onSelect={() => select(kind)}>
    {t(`redis.write.actions.${kind}`)}
  </DropdownMenuItem>;
  return <DropdownMenu>
    <DropdownMenuTrigger asChild><Button type="button" size="sm" variant="outline">{t("redis.write.keyActions")}</Button></DropdownMenuTrigger>
    <DropdownMenuContent align="start">
      {info.keyType === "string" && entry("updateString", !complete)}
      {info.keyType === "hash" && <>
        {entry("addHashField")}
        {fieldBase64 !== undefined && info.fieldFound && entry("updateHashField", !complete)}
        {fieldBase64 !== undefined && info.fieldFound && entry("deleteHashField", !complete, true)}
      </>}
      {entry("setTtl")}
      {entry("persist", info.pttlMs < 0)}
      {entry("deleteKey", false, true)}
    </DropdownMenuContent>
  </DropdownMenu>;
}
