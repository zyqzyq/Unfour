import * as React from "react";
import { AlertCircle, Loader2 } from "lucide-react";
import { useI18n } from "./i18n";
import { cn } from "./utils";

export function EmptyState({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-h-[96px] items-center justify-center rounded-[var(--u-radius-sm)] border border-dashed border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 text-center text-[12px] text-[var(--u-color-text-muted)]",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function LoadingState({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  const { t } = useI18n();
  const content = children ?? t("common.state.loading");

  return (
    <div
      className={cn(
        "flex min-h-[96px] items-center justify-center gap-2 rounded-[var(--u-radius-sm)] border border-dashed border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 text-[12px] text-[var(--u-color-text-muted)]",
        className,
      )}
    >
      <Loader2 className="animate-spin" size={14} />
      {content}
    </div>
  );
}

export function ErrorState({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-h-[96px] items-center justify-center gap-2 rounded-[var(--u-radius-sm)] border border-[color:color-mix(in_srgb,var(--u-color-danger)_34%,var(--u-color-border))] bg-[var(--u-color-danger-soft)] p-3 text-center text-[12px] text-[var(--u-color-danger)]",
        className,
      )}
    >
      <AlertCircle size={14} />
      <span className="min-w-0 select-text">{children}</span>
    </div>
  );
}
