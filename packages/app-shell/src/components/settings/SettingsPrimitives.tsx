import type { ReactNode } from "react";

export function SettingsSectionHeading({
  description,
  title,
}: {
  description: string;
  title: string;
}) {
  return (
    <div>
      <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">{title}</h2>
      <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">{description}</p>
    </div>
  );
}

export function SettingsGroup({
  children,
  description,
  title,
}: {
  children: ReactNode;
  description?: ReactNode;
  title: string;
}) {
  return (
    <section className="space-y-2">
      <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">{title}</h3>
      {description ? (
        <p className="text-[12px] leading-5 text-[var(--u-color-text-muted)]">{description}</p>
      ) : null}
      {children}
    </section>
  );
}

export function SettingsRow({
  control,
  description,
  label,
}: {
  control: ReactNode;
  description: string;
  label: string;
}) {
  return (
    <div className="grid grid-cols-[150px_minmax(0,1fr)] gap-3 border-t border-[var(--u-color-border)] pt-3">
      <div>
        <div className="text-[12px] font-semibold text-[var(--u-color-text)]">{label}</div>
        <div className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {description}
        </div>
      </div>
      <div className="max-w-[260px]">{control}</div>
    </div>
  );
}
