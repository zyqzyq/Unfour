import type * as React from "react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Copy, Download, MoreHorizontal, Save, Trash2, Upload } from "lucide-react";
import { IconButton, cn } from "@unfour/ui";

export function RequestActionsMenu({
  canDelete,
  canDuplicate,
  canExport,
  deleting,
  duplicating,
  importing,
  onDelete,
  onDuplicate,
  onExport,
  onImport,
  onSave,
  saving,
}: {
  canDelete: boolean;
  canDuplicate: boolean;
  canExport: boolean;
  deleting: boolean;
  duplicating: boolean;
  importing: boolean;
  onDelete: () => void;
  onDuplicate: () => void;
  onExport: () => void;
  onImport: () => void;
  onSave: () => void;
  saving: boolean;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <IconButton label="Request actions" tooltip="Request actions">
          <MoreHorizontal size={16} />
        </IconButton>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          className="z-50 w-52 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1 text-[13px] text-[var(--u-color-text)] shadow-lg"
          sideOffset={5}
        >
          <ApiMenuItem disabled={saving} icon={<Save size={14} />} onSelect={onSave}>
            Save request
          </ApiMenuItem>
          <ApiMenuItem
            disabled={!canDuplicate || duplicating}
            icon={<Copy size={14} />}
            onSelect={onDuplicate}
          >
            Duplicate
          </ApiMenuItem>
          <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
          <ApiMenuItem
            disabled={importing}
            icon={<Upload size={14} />}
            onSelect={onImport}
          >
            Import collection
          </ApiMenuItem>
          <ApiMenuItem disabled={!canExport} icon={<Download size={14} />} onSelect={onExport}>
            Export collection
          </ApiMenuItem>
          <DropdownMenu.Separator className="my-1 h-px bg-[var(--u-color-border)]" />
          <ApiMenuItem
            danger
            disabled={!canDelete || deleting}
            icon={<Trash2 size={14} />}
            onSelect={onDelete}
          >
            Delete request
          </ApiMenuItem>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function ApiMenuItem({
  children,
  danger,
  disabled,
  icon,
  onSelect,
}: {
  children: React.ReactNode;
  danger?: boolean;
  disabled?: boolean;
  icon: React.ReactNode;
  onSelect: () => void;
}) {
  return (
    <DropdownMenu.Item
      className={cn(
        "flex h-8 cursor-pointer items-center gap-2 rounded-[var(--u-radius-sm)] px-2 outline-none hover:bg-[var(--u-color-surface-hover)] focus:bg-[var(--u-color-surface-hover)] disabled:pointer-events-none disabled:opacity-50",
        danger && "text-[var(--u-color-danger)]",
      )}
      disabled={disabled}
      onSelect={onSelect}
    >
      {icon}
      {children}
    </DropdownMenu.Item>
  );
}
