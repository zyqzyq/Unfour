import { MoreVertical, Pencil, Trash2 } from "lucide-react";
import type { RedisConnection } from "@unfour/command-client";
import { ContextMenuItem, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, IconButton, useI18n } from "@unfour/ui";

export type RedisTreeActions = {
  open: (connection: RedisConnection) => void;
  edit: (connection: RedisConnection) => void;
  remove: (connection: RedisConnection) => void;
};

export function RedisRowActions({ connection, actions, contextMenu }: {
  connection: RedisConnection;
  actions: RedisTreeActions;
  contextMenu?: boolean;
}) {
  const { t } = useI18n();
  const Item = contextMenu ? ContextMenuItem : DropdownMenuItem;
  const entries = <>
    <Item onSelect={() => actions.edit(connection)}><Pencil size={13} />{t("database.tree.editConnection")}</Item>
    <Item className="text-[var(--u-color-danger)]" onSelect={() => actions.remove(connection)}><Trash2 size={13} />{t("database.tree.deleteConnection")}</Item>
  </>;
  if (contextMenu) return entries;
  return <DropdownMenu>
    <DropdownMenuTrigger asChild><IconButton disableTooltip label={t("database.tree.actionsLabel", { name: connection.name })} size="compact"><MoreVertical size={14} /></IconButton></DropdownMenuTrigger>
    <DropdownMenuContent align="end">{entries}</DropdownMenuContent>
  </DropdownMenu>;
}
