import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "./utils";

const buttonVariants = cva(
  "inline-flex h-[var(--u-size-button)] shrink-0 items-center justify-center gap-2 rounded-[var(--u-radius-sm)] px-3 text-[13px] font-medium transition-colors duration-150 disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]",
  {
    variants: {
      variant: {
        default:
          "border border-[var(--u-color-primary)] bg-[var(--u-color-primary)] text-[var(--u-color-primary-foreground)] hover:bg-[var(--u-color-primary-hover)]",
        secondary:
          "border border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] text-[var(--u-color-text)] hover:bg-[var(--u-color-surface-hover)]",
        ghost:
          "border border-transparent text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
        outline:
          "border border-[var(--u-color-border-strong)] bg-[var(--u-color-surface)] text-[var(--u-color-text)] hover:border-[var(--u-color-border-strong)] hover:bg-[var(--u-color-surface-hover)]",
        danger:
          "border border-[var(--u-color-danger)] bg-[var(--u-color-danger)] text-[var(--u-color-primary-foreground)] hover:bg-[var(--u-color-danger-hover)]",
      },
      size: {
        default: "h-[var(--u-size-button)] px-3",
        icon: "h-[var(--u-size-button)] w-[var(--u-size-button)] px-0",
        sm: "h-[var(--u-size-button-compact)] px-2 text-[12px]",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  };

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ asChild = false, className, size, variant, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ className, size, variant }))}
        ref={ref}
        {...props}
      />
    );
  },
);

Button.displayName = "Button";
