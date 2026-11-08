import * as React from "react";
import { cn } from "./utils";

export function Toolbar({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-h-[var(--u-size-section-toolbar)] shrink-0 items-center justify-between gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function ToolbarGroup({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return <div className={cn("flex min-w-0 items-center gap-1", className)}>{children}</div>;
}
