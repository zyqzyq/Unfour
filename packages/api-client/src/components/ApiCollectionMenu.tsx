import { MoreHorizontal } from "lucide-react";
import { ContextMenuItem, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, useI18n } from "@unfour/ui";

export function ApiCollectionMenu({ context = false, name, onRename, onAddFolder, onExport, onDelete }: {
  context?: boolean;
  name: string;
  onRename: () => void;
  onAddFolder: () => void;
  onExport: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const actions = [
    {key:"rename",onSelect:onRename},
    {key:"addFolder",onSelect:onAddFolder},
    {key:"export",onSelect:onExport},
    {key:"delete",onSelect:onDelete},
  ];
  const Item = context ? ContextMenuItem : DropdownMenuItem;
  const items = actions.map((action) => <Item key={action.key} onSelect={action.onSelect} className={action.key === "delete" ? "text-[var(--u-color-danger)]" : undefined}>{t(`api.collection.${action.key}`)}</Item>);
  if (context) return <>{items}</>;
  return <DropdownMenu>
    <DropdownMenuTrigger asChild>
      <button type="button" aria-label={t("exchange.collectionActions",{name})} className="grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"><MoreHorizontal size={13}/></button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">{items}</DropdownMenuContent>
  </DropdownMenu>;
}
