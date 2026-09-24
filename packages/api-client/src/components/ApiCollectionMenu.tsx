import { MoreHorizontal } from "lucide-react";
import { IconButton, ContextMenuItem, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, useI18n } from "@unfour/ui";

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
      <IconButton label={t("exchange.collectionActions", { name })} size="compact" className="h-6 w-6" disableTooltip><MoreHorizontal size={14} /></IconButton>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">{items}</DropdownMenuContent>
  </DropdownMenu>;
}
