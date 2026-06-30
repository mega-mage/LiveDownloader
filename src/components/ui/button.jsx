import * as React from "react";
import { cn } from "@/lib/utils";

export const Button = React.forwardRef(({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}, ref) => {
  const Comp = asChild ? "span" : "button";
  return (
    <Comp
      className={cn(
        "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer active:scale-98",
        // Variants
        variant === "default" && "bg-primary text-primary-foreground shadow-md hover:opacity-90",
        variant === "destructive" && "bg-destructive text-destructive-foreground shadow-sm hover:opacity-90",
        variant === "outline" && "border border-border bg-transparent shadow-sm hover:bg-secondary hover:text-foreground",
        variant === "secondary" && "bg-secondary text-secondary-foreground shadow-sm hover:opacity-90",
        variant === "ghost" && "hover:bg-secondary hover:text-foreground",
        variant === "link" && "text-primary underline-offset-4 hover:underline",
        // Sizes
        size === "default" && "h-9 px-4 py-2",
        size === "sm" && "h-8 rounded-md px-3 text-xs",
        size === "lg" && "h-10 rounded-md px-8",
        size === "icon" && "h-9 w-9",
        className
      )}
      ref={ref}
      {...props}
    />
  );
});
Button.displayName = "Button";
