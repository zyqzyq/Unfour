import { Children, isValidElement, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { Dialog } from "./dialog-primitives";
import { DialogContent } from "./dialog";
import { Input } from "./input";
import { useI18n } from "./i18n";
import { cn } from "./utils";

export type CommandPaletteItem = {
  id: string;
  label: ReactNode;
  searchText?: string;
  onSelect: () => void;
};

export function CommandPalette({ items, onClose, open }: {
  items: readonly CommandPaletteItem[];
  onClose: () => void;
  open: boolean;
}) {
  return (
    <Dialog onOpenChange={(next) => !next && onClose()} open={open}>
      {open && <CommandPaletteContent items={items} />}
    </Dialog>
  );
}

function CommandPaletteContent({ items }: { items: readonly CommandPaletteItem[] }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [returnFocus] = useState(() => document.activeElement);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeOptionRef = useRef<HTMLButtonElement>(null);
  const listId = useId();
  const terms = query.trim().toLocaleLowerCase().split(/\s+/);
  const filtered = items.filter((item) => {
    const text = item.searchText ?? (labelText(item.label) || item.id);
    return terms.every((term) => text.toLocaleLowerCase().includes(term));
  });
  const index = Math.min(selectedIndex, Math.max(0, filtered.length - 1));
  const selected = filtered[index];

  useEffect(() => {
    activeOptionRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [index, query]);

  return (
    <DialogContent
      aria-describedby={undefined}
      className="top-[12vh] max-h-[76dvh] w-[min(640px,calc(100vw-32px))] translate-y-0"
      onOpenAutoFocus={(event) => {
        event.preventDefault();
        inputRef.current?.focus();
      }}
      onCloseAutoFocus={(event) => {
        event.preventDefault();
        if (returnFocus instanceof HTMLElement && returnFocus.isConnected) returnFocus.focus();
      }}
      title={t("app.commandPalette.open")}
    >
      <div className="shrink-0 border-b border-[var(--u-color-border)] p-2">
        <Input
          aria-activedescendant={selected ? `${listId}-${index}` : undefined}
          aria-autocomplete="list"
          aria-controls={listId}
          aria-expanded="true"
          aria-label={t("app.commandPalette.placeholder")}
          onChange={(event) => { setQuery(event.target.value); setSelectedIndex(0); }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              if (filtered.length) setSelectedIndex((index + (event.key === "ArrowDown" ? 1 : -1) + filtered.length) % filtered.length);
            } else if (event.key === "Enter" && selected) {
              event.preventDefault();
              selected.onSelect();
            }
          }}
          placeholder={t("app.commandPalette.placeholder")}
          ref={inputRef}
          role="combobox"
          value={query}
        />
      </div>
      <div aria-label={t("app.commandPalette.results")} className="min-h-0 overflow-y-auto p-1" id={listId} role="listbox">
        {filtered.map((item, optionIndex) => (
          <button
            aria-selected={index === optionIndex}
            className={cn(
              "flex min-h-8 w-full items-center rounded-[var(--u-radius-sm)] px-2 py-1 text-left text-[13px] text-[var(--u-color-text)] hover:bg-[var(--u-color-surface-hover)]",
              index === optionIndex && "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]",
            )}
            id={`${listId}-${optionIndex}`}
            key={item.id}
            onClick={item.onSelect}
            ref={index === optionIndex ? activeOptionRef : undefined}
            role="option"
            tabIndex={-1}
            type="button"
          >
            {item.label}
          </button>
        ))}
      </div>
      {filtered.length === 0 && (
        <p className="p-3 text-[12px] text-[var(--u-color-text-muted)]" role="status">{t("app.commandPalette.noResults")}</p>
      )}
    </DialogContent>
  );
}

function labelText(node: ReactNode): string {
  return Children.toArray(node).map((child): string => {
    if (typeof child === "string" || typeof child === "number") return String(child);
    return isValidElement<{ children?: ReactNode }>(child) ? labelText(child.props.children) : "";
  }).join(" ");
}
