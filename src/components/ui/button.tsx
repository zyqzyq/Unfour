import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium transition-colors duration-150 disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-700/30 focus-visible:ring-offset-1",
  {
    variants: {
      variant: {
        default: "bg-teal-700 text-white shadow-sm hover:bg-teal-800",
        secondary: "border border-slate-200 bg-slate-100 text-slate-900 hover:bg-slate-200",
        ghost: "text-slate-700 hover:bg-slate-100 hover:text-slate-950",
        outline: "border border-slate-300 bg-white text-slate-800 shadow-xs hover:border-slate-400 hover:bg-slate-50",
      },
      size: {
        default: "h-9 px-3",
        icon: "h-9 w-9 px-0",
        sm: "h-8 px-2.5 text-xs",
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
