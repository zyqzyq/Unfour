import type { ReactNode } from "react";
import { methodBadgeLabel, methodToneClass } from "../model/request-tabs";

export function MethodMeta({ method }: { method: string }) {
  return (
    <span
      className={`w-9 shrink-0 text-left text-[10px] font-bold uppercase tabular-nums ${methodToneClass(method)}`}
    >
      {methodBadgeLabel(method)}
    </span>
  );
}

export function SidebarEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
      {children}
    </div>
  );
}
