import * as React from "react";
import { cn } from "../../lib/utils";

type BadgeProps = React.HTMLAttributes<HTMLSpanElement> & {
  tone?: "neutral" | "green" | "amber" | "red" | "teal";
};

const toneClass: Record<NonNullable<BadgeProps["tone"]>, string> = {
  amber: "bg-amber-50 text-amber-800 ring-amber-200",
  green: "bg-emerald-50 text-emerald-800 ring-emerald-200",
  neutral: "bg-slate-100 text-slate-700 ring-slate-200",
  red: "bg-rose-50 text-rose-800 ring-rose-200",
  teal: "bg-teal-50 text-teal-800 ring-teal-200",
};

export function Badge({ className, tone = "neutral", ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded px-2 py-0.5 text-xs font-medium leading-5 ring-1 ring-inset",
        toneClass[tone],
        className,
      )}
      {...props}
    />
  );
}
