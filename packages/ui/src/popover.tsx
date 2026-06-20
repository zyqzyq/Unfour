import * as PopoverPrimitive from "@radix-ui/react-popover";
import * as React from "react";
import { cn } from "./utils";

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;
export const PopoverAnchor = PopoverPrimitive.Anchor;
export const PopoverClose = PopoverPrimitive.Close;

const popoverContent =
  "z-50 overflow-hidden rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] text-[12px] text-[var(--u-color-text)] shadow-lg focus-visible:outline-none";

export function PopoverContent({
  align = "start",
  className,
  sideOffset = 4,
  ...props
}: React.ComponentProps<typeof PopoverPrimitive.Content>) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        align={align}
        className={cn(popoverContent, className)}
        sideOffset={sideOffset}
        {...props}
      />
    </PopoverPrimitive.Portal>
  );
}
