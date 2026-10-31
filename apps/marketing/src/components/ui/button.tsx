import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

/** The site's buttons, shared by this component and link-button.astro. */
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-full border text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:ring-3 focus-visible:ring-foreground/30 active:not-aria-[haspopup]:translate-y-px [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-foreground text-background hover:bg-foreground/80 aria-expanded:bg-foreground/80",
        outline: "border-border hover:bg-muted aria-expanded:bg-muted",
        ghost:
          "border-transparent text-muted-foreground hover:bg-muted hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground",
      },
      size: {
        sm: "h-8 px-3",
        default: "h-9 px-3",
        lg: "h-10 px-4",
        "icon-lg": "size-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant,
  size,
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
