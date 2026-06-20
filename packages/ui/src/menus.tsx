import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import * as React from "react";
import { cn } from "./utils";

const menuContent =
  "z-50 min-w-[180px] overflow-hidden rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-1 text-[12px] text-[var(--u-color-text)] shadow-lg";
const menuItem =
  "flex h-7 cursor-default select-none items-center gap-2 rounded-[var(--u-radius-sm)] px-2 outline-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-[var(--u-color-surface-hover)]";

export function DropdownMenuContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Content>) {
  return <DropdownMenuPrimitive.Content className={cn(menuContent, className)} sideOffset={4} {...props} />;
}
export function DropdownMenuItem({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Item>) {
  return <DropdownMenuPrimitive.Item className={cn(menuItem, className)} {...props} />;
}

type ContextMenuState = {
  close: () => void;
  open: boolean;
  openAt: (position: { x: number; y: number }) => void;
  position: { x: number; y: number };
};

const ContextMenuContext = React.createContext<ContextMenuState | null>(null);

export function ContextMenu({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false);
  const [position, setPosition] = React.useState({ x: 0, y: 0 });

  React.useEffect(() => {
    if (!open) {
      return;
    }

    function close() {
      setOpen(false);
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        close();
      }
    }

    window.addEventListener("click", close);
    window.addEventListener("contextmenu", close);
    window.addEventListener("keydown", closeOnEscape);

    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("contextmenu", close);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  const value = React.useMemo<ContextMenuState>(
    () => ({
      close: () => setOpen(false),
      open,
      openAt: (nextPosition) => {
        setPosition(nextPosition);
        setOpen(true);
      },
      position,
    }),
    [open, position],
  );

  return (
    <ContextMenuContext.Provider value={value}>
      {children}
    </ContextMenuContext.Provider>
  );
}

export function ContextMenuTrigger({
  asChild,
  children,
}: {
  asChild?: boolean;
  children: React.ReactElement<{ onContextMenu?: React.MouseEventHandler }>;
}) {
  const context = React.useContext(ContextMenuContext);

  if (!context) {
    return children;
  }

  const handleContextMenu: React.MouseEventHandler = (event) => {
    children.props.onContextMenu?.(event);
    if (event.defaultPrevented) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    context.openAt({ x: event.clientX, y: event.clientY });
  };

  if (asChild) {
    return React.cloneElement(children, { onContextMenu: handleContextMenu });
  }

  return (
    <span className="contents" onContextMenu={handleContextMenu}>
      {children}
    </span>
  );
}

export function ContextMenuContent({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  const context = React.useContext(ContextMenuContext);
  const ref = React.useRef<HTMLDivElement>(null);
  const [offset, setOffset] = React.useState({ x: 0, y: 0 });

  // Keep the menu inside the viewport instead of overflowing past the right
  // edge or below the status bar when opened near a window boundary.
  React.useLayoutEffect(() => {
    if (!context?.open || !ref.current) {
      return;
    }
    const rect = ref.current.getBoundingClientRect();
    const margin = 8;
    // Measure against the raw click position and the menu size so the result is
    // independent of any offset already applied this open.
    const overflowX = Math.max(0, context.position.x + rect.width + margin - window.innerWidth);
    const overflowY = Math.max(0, context.position.y + rect.height + margin - window.innerHeight);
    if (overflowX !== offset.x || overflowY !== offset.y) {
      setOffset({ x: overflowX, y: overflowY });
    }
  }, [context?.open, context?.position, offset.x, offset.y]);

  if (!context?.open) {
    return null;
  }

  return (
    <div
      className={cn(menuContent, className)}
      onClick={(event) => event.stopPropagation()}
      ref={ref}
      role="menu"
      style={{
        left: Math.max(8, context.position.x - offset.x),
        position: "fixed",
        top: Math.max(8, context.position.y - offset.y),
      }}
    >
      {children}
    </div>
  );
}

export function ContextMenuItem({
  children,
  className,
  disabled,
  onSelect,
  tone = "default",
}: {
  children?: React.ReactNode;
  className?: string;
  disabled?: boolean;
  onSelect?: () => void;
  tone?: "default" | "danger";
}) {
  const context = React.useContext(ContextMenuContext);

  return (
    <button
      className={cn(
        "w-full text-left",
        menuItem,
        tone === "danger" &&
          "text-[var(--u-color-danger)] data-[highlighted]:bg-[var(--u-color-danger-soft)] hover:bg-[var(--u-color-danger-soft)]",
        className,
      )}
      disabled={disabled}
      onClick={() => {
        onSelect?.();
        context?.close();
      }}
      role="menuitem"
      type="button"
    >
      {children}
    </button>
  );
}
