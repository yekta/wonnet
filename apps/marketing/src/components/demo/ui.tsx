import {
  CheckIcon,
  FileImageIcon,
  FileTextIcon,
  StarIcon,
  XIcon,
  type LucideIcon,
} from "lucide-react"
import { useEffect, useState, type ComponentProps, type ReactNode } from "react"

/** The apps' components (packages/apple/Sources/MailUI/Shared/Views/UI), drawn with tokens.css. */

export function cn(...names: (string | false | null | undefined)[]) {
  return names.filter(Boolean).join(" ")
}

/** A shadow of the size in tokens.json, in the colour shadow at one of its opacities. */
export function shadow(size: "sm" | "md" | "lg" | "xl", strength = "shadow") {
  return `var(--shadow-${size}) color-mix(in srgb, var(--shadow) calc(var(--opacity-${strength}) * 100%), transparent)`
}

/** A button's hit area: half the button gap on each side of its face, so hit areas touch. Its
 * face hovers in `--button-hover` and is circled in `--button-ring`, which the card and the
 * popover set to their own. */
const HIT =
  "group/button flex shrink-0 px-[3px] disabled:pointer-events-none disabled:opacity-(--opacity-disabled)"

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
      className={cn(HIT, className)}
      {...props}
    >
      <span
        className={cn(
          "flex size-9 items-center justify-center rounded-full text-(--muted-foreground) group-hover/button:bg-(--button-hover) group-hover/button:text-(--foreground)",
          circled && "border border-(--button-ring)"
        )}
      >
        <Icon className="size-4" strokeWidth={1.75} />
      </span>
    </button>
  )
}

export function ActionButton({
  icon: Icon,
  title,
  variant = "outline",
  size = "regular",
  className,
  ...props
}: {
  icon?: LucideIcon
  title: string
  variant?: "outline" | "primary" | "ghost"
  size?: "small" | "regular"
} & ComponentProps<"button">) {
  return (
    <button type="button" className={cn(HIT, className)} {...props}>
      <span
        className={cn(
          "flex items-center gap-1.5 rounded-full border text-[13px] font-medium",
          size === "small" ? "h-7 px-3.5" : "h-9 px-[18px]",
          variant === "outline"
            ? "border-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-border)*100%),transparent)] text-(--primary) group-hover/button:bg-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-tint)*100%),transparent)]"
            : variant === "ghost"
              ? "border-transparent text-(--muted-foreground) group-hover/button:bg-(--button-hover) group-hover/button:text-(--foreground)"
              : "border-transparent bg-(--primary) text-(--primary-foreground) group-hover/button:bg-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-lit)*100%),transparent)]"
        )}
      >
        {Icon && (
          <Icon
            className={cn(
              "-ml-[3px]",
              size === "small" ? "size-3" : "size-3.5"
            )}
            strokeWidth={2}
          />
        )}
        {title}
      </span>
    </button>
  )
}

/** A button that is only its content (a sender), on a rounded fill when
 * hovered, never under 28px. `flush` gives the fill room all round and pulls it out by as much. */
export function PlainButton({
  flush,
  className,
  ...props
}: { flush?: boolean } & ComponentProps<"button">) {
  return (
    <button
      type="button"
      className={cn(
        "flex min-h-7 items-center justify-center hover:bg-(--button-hover)",
        flush ? "-m-1.5 rounded-md p-1.5" : "min-w-7 rounded-sm px-1",
        className
      )}
      {...props}
    />
  )
}

/** A thread's star: lit in the star colour when starred, and on hover, over a wash of it. */
export function StarButton({
  starred,
  ...props
}: { starred: boolean } & ComponentProps<"button">) {
  return (
    <button
      type="button"
      aria-label={starred ? "Unstar" : "Star"}
      title={starred ? "Unstar" : "Star"}
      className={cn(
        "flex min-h-7 min-w-7 items-center justify-center rounded-sm px-1 hover:bg-[color-mix(in_srgb,var(--star)_calc(var(--opacity-color-tint)*100%),transparent)] hover:text-(--star)",
        starred ? "text-(--star)" : "text-(--muted-stronger-foreground)"
      )}
      {...props}
    >
      <StarIcon
        className="size-5"
        strokeWidth={1.75}
        fill={starred ? "currentColor" : "none"}
      />
    </button>
  )
}

/** The circle that selects threads: empty, or filled in the primary colour with a check. */
export function SelectCircle({
  checked,
  label,
  className,
  ...props
}: {
  checked: boolean
  label: string
} & ComponentProps<"button">) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "group/circle flex size-9 shrink-0 items-center justify-center",
        className
      )}
      {...props}
    >
      <span
        className={cn(
          "flex size-[18px] items-center justify-center rounded-full",
          checked
            ? "bg-(--primary) text-(--primary-foreground)"
            : "border-[1.5px] border-(--border) group-hover/circle:border-(--muted-foreground)"
        )}
      >
        {checked && <CheckIcon className="size-3" strokeWidth={2.5} />}
      </span>
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

/** A message's file under it, as Gmail shows them: its kind's icon over its name, kind and size.
 * Drawn as on the thread's card. */
export function AttachmentCard({
  name,
  size,
  onClick,
}: {
  name: string
  size: string
  onClick: () => void
}) {
  const type = name.split(".").pop()?.toUpperCase() ?? ""
  const Icon = ["JPG", "JPEG", "PNG", "GIF", "HEIC"].includes(type)
    ? FileImageIcon
    : FileTextIcon
  return (
    <button
      type="button"
      title={`Open ${name}`}
      onClick={onClick}
      className="group/card flex w-[168px] flex-col overflow-hidden rounded-md border border-(--border-card) text-left"
    >
      <span className="flex h-24 items-center justify-center bg-(--card-larger-accent) group-hover/card:bg-(--card-larger-accent-stronger)">
        <Icon
          className="size-7 text-(--muted-stronger-foreground)"
          strokeWidth={1.5}
        />
      </span>
      <span className="flex items-center gap-1.5 border-t border-(--border-card) bg-(--card-larger) px-2 py-1.5 group-hover/card:bg-(--card-larger-accent)">
        <Icon
          className="size-3.5 shrink-0 text-(--muted-foreground)"
          strokeWidth={1.75}
        />
        <span className="min-w-0">
          <span className="block truncate text-[12px] text-(--foreground)">
            {name}
          </span>
          <span className="block text-[11px] text-(--muted-stronger-foreground)">
            {type} · {size}
          </span>
        </span>
      </span>
    </button>
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
    <button type="button" onClick={onClick} className="group/tab flex px-0.5">
      <span
        className={cn(
          "flex h-9 items-center gap-1.5 rounded-md px-3 text-[13px]",
          chosen
            ? "bg-(--background-accent-stronger) text-(--foreground)"
            : "text-(--muted-foreground) group-hover/tab:bg-(--background-accent) group-hover/tab:text-(--foreground)"
        )}
      >
        <Icon className="size-3.5" strokeWidth={1.75} />
        {title}
        {count > 0 && (
          <span
            className={cn(
              "text-[11px]",
              chosen
                ? "text-(--muted-foreground)"
                : "text-(--muted-stronger-foreground)"
            )}
          >
            {count}
          </span>
        )}
      </span>
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
        className="relative w-[400px] overflow-hidden rounded-lg border border-(--border) bg-(--background)"
        style={{ boxShadow: shadow("xl", "shadow-stronger") }}
      >
        <div className="relative flex h-[52px] items-center justify-center border-b border-(--border)">
          <IconButton
            icon={XIcon}
            label="Close (Esc)"
            circled={false}
            onClick={onClose}
            className="absolute left-[5px]"
          />
          <span className="text-[13px] font-semibold text-(--foreground)">
            {title}
          </span>
        </div>
        {children}
      </div>
    </div>
  )
}

/** The note at the bottom: a line of text and maybe an action. One that counts down (a message
 * about to be sent) starts with a ring that empties until it is done. The ring and the action are
 * 32px capsules inset 6px from the toast's edge on every side (5px of padding inside its 1px
 * border), so their ends are concentric with the toast's. */
export function Toast({
  message,
  action,
  onAction,
  countdown,
}: {
  message: string
  action?: string
  onAction?: () => void
  /** When it started, in milliseconds since the epoch, and how many seconds it lasts. */
  countdown?: { start: number; duration: number }
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-5 z-50 flex justify-center">
      <div
        className="pointer-events-auto flex h-11 items-center rounded-full border border-(--border-popover) bg-(--popover) p-[5px]"
        style={{ boxShadow: shadow("md", "shadow-stronger") }}
      >
        {countdown && <Countdown {...countdown} />}
        <span
          className={cn(
            "text-[13px] whitespace-nowrap text-(--foreground)",
            countdown ? "pl-2.5" : "pl-3",
            action ? "pr-5" : "pr-3"
          )}
        >
          {message}
        </span>
        {action && (
          <button
            type="button"
            onClick={onAction}
            className="flex h-8 min-w-[51px] items-center justify-center rounded-full border border-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-border)*100%),transparent)] px-4 text-[13px] font-medium text-(--primary) hover:bg-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-tint)*100%),transparent)]"
          >
            {action}
          </button>
        )}
      </div>
    </div>
  )
}

const RING_LINE = 2.5
const RING_RADIUS = (32 - RING_LINE) / 2
const RING_LENGTH = 2 * Math.PI * RING_RADIUS

/** The seconds left in a ring that empties, from the top and clockwise, as they run out. */
function Countdown({ start, duration }: { start: number; duration: number }) {
  const [left, setLeft] = useState(duration)
  useEffect(() => {
    let frame = 0
    const tick = () => {
      const remaining = Math.max(0, duration - (Date.now() - start) / 1000)
      setLeft(remaining)
      if (remaining > 0) frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
  }, [start, duration])

  const fraction = duration > 0 ? left / duration : 0
  return (
    <span className="relative flex size-8 shrink-0 items-center justify-center">
      <svg
        className="absolute inset-0 -rotate-90"
        viewBox="0 0 32 32"
        aria-hidden
      >
        <circle
          cx={16}
          cy={16}
          r={RING_RADIUS}
          fill="none"
          strokeWidth={RING_LINE}
          className="stroke-(--popover-accent-stronger)"
        />
        {fraction > 0 && (
          <circle
            cx={16}
            cy={16}
            r={RING_RADIUS}
            fill="none"
            strokeWidth={RING_LINE}
            strokeLinecap="round"
            strokeDasharray={`${fraction * RING_LENGTH} ${RING_LENGTH}`}
            className="stroke-(--foreground)"
          />
        )}
      </svg>
      <span className="relative text-[13px] font-medium text-(--foreground) tabular-nums">
        {Math.ceil(left)}
      </span>
    </span>
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
