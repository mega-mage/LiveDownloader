import * as React from "react";
import { cn } from "@/lib/utils";

export const Select = React.forwardRef(({ className, ...props }, ref) => {
  return (
    <select
      ref={ref}
      className={cn(
        "flex h-9 w-full rounded-md border border-border bg-card px-3 py-1 text-sm shadow-sm transition-all duration-200 placeholder:text-muted-foreground focus:outline-none focus:border-primary disabled:cursor-not-allowed disabled:opacity-50 text-foreground cursor-pointer",
        className
      )}
      {...props}
    />
  );
});
Select.displayName = "Select";
