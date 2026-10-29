import { useRef } from "react";
import { Search, X } from "lucide-react";
import { IconButton } from "./icon-button";
import { Input } from "./input";
import { useI18n } from "./i18n";

/** Compact local filter control; the owning feature supplies matching semantics. */
export function SidebarSearch({ label, placeholder, value, onChange }: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useI18n();
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <div className="relative mx-1 my-1 shrink-0">
      <Search aria-hidden size={13} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-[var(--u-color-text-muted)]" />
      <Input
        aria-label={label}
        className="h-7 bg-[var(--u-color-surface-subtle)] pl-7 pr-7 text-[12px]"
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        ref={inputRef}
        value={value}
      />
      {value && (
        <IconButton
          className="absolute right-0 top-0"
          label={t("common.search.clear")}
          onClick={() => { onChange(""); inputRef.current?.focus(); }}
          size="compact"
        >
          <X size={13} />
        </IconButton>
      )}
    </div>
  );
}
