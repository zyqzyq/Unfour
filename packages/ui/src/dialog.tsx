import * as DialogPrimitive from "@radix-ui/react-dialog";
import * as React from "react";
import { X } from "lucide-react";
import { cn } from "./utils";
import { IconButton } from "./icon-button";

export function DialogContent({
  children,
  className,
  title,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content> & {
  title?: string;
}) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="u-dialog-overlay fixed inset-0 z-50 bg-[var(--u-color-overlay)] backdrop-blur-[3px]" />
      <DialogPrimitive.Content
        className={cn(
          "u-dialog-content fixed left-1/2 top-1/2 z-50 flex max-h-[min(760px,calc(100vh-48px))] w-[min(560px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[var(--u-radius-lg)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] text-[13px] text-[var(--u-color-text)] shadow-lg focus-visible:outline-none",
          className,
        )}
        {...props}
      >
        {title && <DialogTitle className="sr-only">{title}</DialogTitle>}
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

export function DialogHeader({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-h-[56px] shrink-0 items-center justify-between gap-3 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-5 py-3",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function DialogTitle({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Title>) {
  return (
    <DialogPrimitive.Title
      className={cn("truncate text-[15px] font-semibold tracking-tight text-[var(--u-color-text)]", className)}
      {...props}
    />
  );
}

export function DialogDescription({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Description>) {
  return (
    <DialogPrimitive.Description
      className={cn("text-[12px] text-[var(--u-color-text-muted)]", className)}
      {...props}
    />
  );
}

export function DialogBody({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return <div className={cn("min-h-0 overflow-y-auto p-5", className)}>{children}</div>;
}

export function DialogFooter({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center justify-end gap-2 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-5 py-3",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function DialogXClose({
  label = "Close dialog",
  ...props
}: Omit<React.ComponentProps<typeof IconButton>, "label"> & { label?: string }) {
  return (
    <DialogPrimitive.Close asChild>
      <IconButton label={label} {...props}>
        <X size={14} />
      </IconButton>
    </DialogPrimitive.Close>
  );
}
