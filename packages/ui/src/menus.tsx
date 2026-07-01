import * as ContextMenuPrimitive from "@radix-ui/react-context-menu";
import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import * as React from "react";
import { cn } from "./utils";

const menuContent =
  "z-50 min-w-[180px] overflow-hidden rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1 text-[12px] font-normal text-[var(--u-color-text)] shadow-lg";
const menuItem =
  "flex h-7 cursor-default select-none items-center gap-2 rounded-[var(--u-radius-sm)] px-2 outline-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-[var(--u-color-surface-hover)]";

export function DropdownMenuContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Content>) {
  return <DropdownMenuPrimitive.Content className={cn(menuContent, className)} sideOffset={4} {...props} />;
}

export function DropdownMenuItem({
  children,
  className,
  shortcut,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Item> & {
  /** Optional shortcut hint displayed at the right edge (e.g. "Ctrl+S"). */
  shortcut?: string;
}) {
  return (
    <DropdownMenuPrimitive.Item className={cn(menuItem, className)} {...props}>
      {children}
      {shortcut && (
        <span className="ml-auto shrink-0 text-[11px] text-[var(--u-color-text-soft)]">{shortcut}</span>
      )}
    </DropdownMenuPrimitive.Item>
  );
}

export const ContextMenu = ContextMenuPrimitive.Root;

export function ContextMenuTrigger({
  asChild,
  children,
}: {
  asChild?: boolean;
  children: React.ReactNode;
}) {
  return <ContextMenuPrimitive.Trigger asChild={asChild}>{children}</ContextMenuPrimitive.Trigger>;
}

export function ContextMenuContent({
  className,
  ...props
}: React.ComponentProps<typeof ContextMenuPrimitive.Content>) {
  return (
    <ContextMenuPrimitive.Portal>
      <ContextMenuPrimitive.Content className={cn(menuContent, className)} collisionPadding={8} {...props} />
    </ContextMenuPrimitive.Portal>
  );
}

export function ContextMenuSeparator({
  className,
  ...props
}: React.ComponentProps<typeof ContextMenuPrimitive.Separator>) {
  return <ContextMenuPrimitive.Separator className={cn("my-1 h-px bg-[var(--u-color-border)]", className)} {...props} />;
}

export function ContextMenuItem({
  children,
  className,
  disabled,
  onSelect,
  shortcut,
  tone = "default",
  ...props
}: Omit<React.ComponentProps<typeof ContextMenuPrimitive.Item>, "onSelect"> & {
  onSelect?: (event: Event) => void;
  /** Optional shortcut hint displayed at the right edge (e.g. "Ctrl+S"). */
  shortcut?: string;
  tone?: "default" | "danger";
}) {
  return (
    <ContextMenuPrimitive.Item
      className={cn(
        menuItem,
        tone === "danger" && "text-[var(--u-color-danger)] data-[highlighted]:bg-[var(--u-color-danger-soft)]",
        className,
      )}
      disabled={disabled}
      onSelect={onSelect}
      {...props}
    >
      {children}
      {shortcut && (
        <span className="ml-auto shrink-0 text-[11px] text-[var(--u-color-text-soft)]">{shortcut}</span>
      )}
    </ContextMenuPrimitive.Item>
  );
}
