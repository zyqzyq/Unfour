import * as React from "react";
import { cn } from "./utils";

export type StatusTone = "neutral" | "success" | "warning" | "danger";

const toneClass: Record<StatusTone, string> = {
  danger: "bg-[var(--u-color-danger-soft)] text-[var(--u-color-danger)]",
  neutral: "bg-[var(--u-color-surface-muted)] text-[var(--u-color-text-muted)]",
  success: "bg-[var(--u-color-success-soft)] text-[var(--u-color-success)]",
  warning: "bg-[var(--u-color-warning-soft)] text-[var(--u-color-warning)]",
};

export function StatusBadge({
  children,
  className,
  tone = "neutral",
}: {
  children: React.ReactNode;
  className?: string;
  tone?: StatusTone;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-5 max-w-full items-center rounded-[var(--u-radius-sm)] px-1.5 text-[11px] font-medium",
        toneClass[tone],
        className,
      )}
    >
      <span className="truncate">{children}</span>
    </span>
  );
}

export type ConnectionStatusValue =
  | "connected"
  | "connecting"
  | "reconnecting"
  | "disconnected"
  | "error"
  | "closed"
  | "unknown";

const dotToneClass: Record<StatusTone, string> = {
  danger: "bg-[var(--u-color-danger)]",
  neutral: "bg-[var(--u-color-text-soft)]",
  success: "bg-[var(--u-color-success)]",
  warning: "bg-[var(--u-color-warning)]",
};

function connectionTone(status: ConnectionStatusValue): StatusTone {
  switch (status) {
    case "connected":
      return "success";
    case "connecting":
    case "reconnecting":
      return "warning";
    case "error":
      return "danger";
    default:
      return "neutral";
  }
}

export function ConnectionStatus({
  connected,
  dotOnly,
  label,
  pulse: pulseProp,
  status,
  variant = "badge",
}: {
  connected?: boolean;
  /** Render only the status dot; the label is exposed via `title`. Implies `variant="dot"`. */
  dotOnly?: boolean;
  label?: string;
  /** Force the pulse animation on (e.g. a failed connection that needs attention). Falls back to pulsing only while connecting/reconnecting. */
  pulse?: boolean;
  status?: ConnectionStatusValue;
  /** `"badge"` (default) keeps the legacy pill look; `"dot"` renders a colored dot + label. */
  variant?: "badge" | "dot";
}) {
  const resolvedStatus = status ?? (connected ? "connected" : "disconnected");
  const tone = connectionTone(resolvedStatus);
  const text = label ?? resolvedStatus;
  const pulse =
    Boolean(pulseProp) || resolvedStatus === "connecting" || resolvedStatus === "reconnecting";

  if (variant === "dot" || dotOnly) {
    return (
      <span
        className={cn(
          "inline-flex items-center gap-1.5 text-[11px] font-medium",
          tone === "success" && "text-[var(--u-color-success)]",
          tone === "warning" && "text-[var(--u-color-warning)]",
          tone === "danger" && "text-[var(--u-color-danger)]",
          tone === "neutral" && "text-[var(--u-color-text-soft)]",
        )}
        title={text}
      >
        <span
          className={cn(
            "h-[7px] w-[7px] shrink-0 rounded-full",
            dotToneClass[tone],
            pulse && "animate-pulse",
          )}
        />
        {!dotOnly && <span className="truncate">{text}</span>}
      </span>
    );
  }

  return (
    <span className={pulse ? "inline-flex animate-pulse" : undefined}>
      <StatusBadge tone={tone}>{text}</StatusBadge>
    </span>
  );
}
