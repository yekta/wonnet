import { XIcon, type LucideIcon } from "lucide-react"
import type { ComponentProps, ReactNode } from "react"

/** The apps' components (packages/apple/Sources/MailUI/Shared/Views/UI), drawn with tokens.css. */

export function cn(...names: (string | false | null | undefined)[]) {
  return names.filter(Boolean).join(" ")
}

/** A shadow of the size in tokens.json, in the colour shadow at one of its opacities. */
export function shadow(size: "sm" | "md" | "lg" | "xl", strength = "shadow") {
  return `var(--shadow-${size}) color-mix(in srgb, var(--shadow) calc(var(--opacity-${strength}) * 100%), transparent)`
}

export function IconButton({
  icon: Icon,
  label,
  circled = true,
  className,
  ...props
}: {
  icon: LucideIcon
  label: string
  circled?: boolean
} & ComponentProps<"button">) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "flex shrink-0 items-center justify-center rounded-full text-(--muted-foreground) hover:bg-(--background-accent) hover:text-(--foreground)",
        circled ? "size-9 border border-(--border)" : "size-7",
        className
      )}
      {...props}
    >
      <Icon className="size-4" strokeWidth={1.75} />
    </button>
  )
}

export function ActionButton({
  icon: Icon,
  title,
  variant = "outline",
  className,
  ...props
}: {
  icon?: LucideIcon
  title: string
  variant?: "outline" | "primary"
} & ComponentProps<"button">) {
  return (
    <button
      type="button"
      className={cn(
        "flex h-9 shrink-0 items-center gap-1.5 rounded-full border px-4 text-[13px] font-medium",
        variant === "outline"
          ? "border-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-border)*100%),transparent)] text-(--primary) hover:bg-(--background-accent)"
          : "border-transparent bg-(--primary) text-(--primary-foreground) hover:opacity-(--opacity-lit) disabled:opacity-(--opacity-disabled)",
        className
      )}
      {...props}
    >
      {Icon && <Icon className="size-3.5" strokeWidth={2} />}
      {title}
    </button>
  )
}

export function Chip({
  icon: Icon,
  children,
}: {
  icon: LucideIcon
  children: ReactNode
}) {
  return (
    <span className="inline-flex h-7 items-center gap-1.5 rounded-full bg-(--card-larger-accent) px-2.5 text-[12px] text-(--foreground)">
      <Icon className="size-3.5 text-(--muted-foreground)" strokeWidth={1.75} />
      {children}
    </span>
  )
}

/** "Inbox 5": the mailboxes pinned to the top bar, the open one filled. */
export function PillTab({
  icon: Icon,
  title,
  count,
  chosen,
  onClick,
}: {
  icon: LucideIcon
  title: string
  count: number
  chosen: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex h-9 items-center gap-1.5 rounded-sm px-3 text-[13px] hover:bg-(--background-accent)",
        chosen
          ? "bg-(--background-accent) text-(--foreground)"
          : "text-(--muted-foreground) hover:text-(--foreground)"
      )}
    >
      <Icon className="size-3.5" strokeWidth={1.75} />
      {title}
      {count > 0 && (
        <span className="text-[11px] text-(--muted-stronger-foreground)">
          {count}
        </span>
      )}
    </button>
  )
}

export function Avatar({
  initials,
  color,
  size = 34,
}: {
  initials: string
  color: string
  size?: number
}) {
  return (
    <span
      className="flex shrink-0 items-center justify-center rounded-full text-[12px] font-semibold text-(--primary-foreground)"
      style={{ width: size, height: size, background: color }}
    >
      {initials}
    </span>
  )
}

/** A sheet over the window: a title between close and nothing, then its content. */
export function Sheet({
  title,
  onClose,
  children,
}: {
  title: string
  onClose: () => void
  children: ReactNode
}) {
  return (
    <div className="absolute inset-0 z-40 flex items-start justify-center pt-[180px]">
      <div
        className="absolute inset-0 bg-(--overlay) opacity-(--opacity-overlay)"
        onClick={onClose}
      />
      <div
        className="relative w-[400px] overflow-hidden rounded-lg border border-(--border-popover) bg-(--popover)"
        style={{ boxShadow: shadow("xl", "shadow-stronger") }}
      >
        <div className="relative flex h-[52px] items-center justify-center border-b border-(--border-popover)">
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="absolute left-3 flex size-7 items-center justify-center rounded-full text-(--muted-foreground) hover:bg-(--popover-accent) hover:text-(--foreground)"
          >
            <XIcon className="size-4" strokeWidth={1.75} />
          </button>
          <span className="text-[13px] font-semibold text-(--foreground)">
            {title}
          </span>
        </div>
        {children}
      </div>
    </div>
  )
}

export function Toast({
  message,
  action,
  onAction,
}: {
  message: string
  action?: string
  onAction?: () => void
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-5 z-50 flex justify-center">
      <div
        className="pointer-events-auto flex h-11 items-center gap-4 rounded-full bg-(--popover) px-[18px] text-[13px] text-(--foreground)"
        style={{ boxShadow: shadow("md", "shadow-stronger") }}
      >
        {message}
        {action && (
          <button
            type="button"
            onClick={onAction}
            className="font-medium text-(--primary) hover:opacity-(--opacity-lit)"
          >
            {action}
          </button>
        )}
      </div>
    </div>
  )
}

export function TrafficLights() {
  return (
    <div className="flex gap-2">
      {["#ff5f57", "#febc2e", "#28c840"].map((color) => (
        <span
          key={color}
          className="size-3 rounded-full"
          style={{ background: color }}
        />
      ))}
    </div>
  )
}
