import {
  ArchiveIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  FileIcon,
  InboxIcon,
  SendIcon,
  SettingsIcon,
  ShieldIcon,
  StarIcon,
  Trash2Icon,
  type LucideIcon,
} from "lucide-react"
import { useState, type ReactNode } from "react"
import { ACCOUNTS, MAILBOXES, snoozeTimes, snoozeDate } from "./mail"
import type { Mailbox } from "./mail"
import { Sheet, cn } from "./ui"

const ICONS: Record<string, LucideIcon> = {
  inbox: InboxIcon,
  starred: StarIcon,
  snoozed: ClockIcon,
  sent: SendIcon,
  drafts: FileIcon,
  archive: ArchiveIcon,
  spam: ShieldIcon,
  trash: Trash2Icon,
}

/** The mailboxes of every account together, then each account with its colour, then settings,
 * sliding over the page from the left. */
export function Sidebar({
  open,
  mailbox,
  account,
  count,
  onPick,
  onClose,
  onSettings,
}: {
  open: boolean
  mailbox: Mailbox
  account: number | null
  count: (mailbox: Mailbox, account: number | null) => number
  onPick: (mailbox: Mailbox, account: number | null) => void
  onClose: () => void
  onSettings: () => void
}) {
  const [expanded, setExpanded] = useState<number[]>([])

  return (
    <>
      <div
        className={cn(
          "absolute inset-0 z-20 bg-(--overlay) transition-opacity duration-200",
          open ? "opacity-(--opacity-overlay)" : "pointer-events-none opacity-0"
        )}
        onClick={onClose}
      />
      <aside
        className={cn(
          "absolute inset-y-0 left-0 z-20 flex w-[300px] flex-col border-r border-(--border) bg-(--background) pt-[52px] transition-transform duration-250 ease-out",
          !open && "-translate-x-full"
        )}
      >
        <div className="flex-1 overflow-y-auto px-2">
          {MAILBOXES.map((one) => (
            <NavRow
              key={one.id}
              icon={ICONS[one.id]}
              title={one.name}
              count={count(one.id, null)}
              selected={account === null && mailbox === one.id}
              onClick={() => onPick(one.id, null)}
            />
          ))}
          <p className="px-[18px] pt-[22px] pb-1.5 text-[14px] font-semibold text-(--foreground)">
            Accounts
          </p>
          {ACCOUNTS.map((one, index) => {
            const shown = expanded.includes(index)
            const Chevron = shown ? ChevronDownIcon : ChevronRightIcon
            return (
              <div key={one.email}>
                <NavRow
                  title={one.name}
                  dot={`var(--${one.color})`}
                  selected={false}
                  onClick={() =>
                    setExpanded(
                      shown
                        ? expanded.filter((other) => other !== index)
                        : [...expanded, index]
                    )
                  }
                >
                  <Chevron
                    className="size-3 text-(--muted-strongest-foreground)"
                    strokeWidth={2}
                  />
                </NavRow>
                {shown &&
                  MAILBOXES.map((box) => (
                    <NavRow
                      key={box.id}
                      icon={ICONS[box.id]}
                      title={box.name}
                      indent
                      count={count(box.id, index)}
                      selected={account === index && mailbox === box.id}
                      onClick={() => onPick(box.id, index)}
                    />
                  ))}
              </div>
            )
          })}
        </div>
        <div className="border-t border-(--border) px-2 py-0.5">
          <NavRow
            icon={SettingsIcon}
            title="Settings"
            selected={false}
            onClick={onSettings}
          />
        </div>
      </aside>
    </>
  )
}

function NavRow({
  icon: Icon,
  dot,
  title,
  count = 0,
  indent = false,
  selected,
  onClick,
  children,
}: {
  icon?: LucideIcon
  dot?: string
  title: string
  count?: number
  indent?: boolean
  selected: boolean
  onClick: () => void
  children?: ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "my-px flex h-9 w-full items-center gap-2.5 rounded-sm pr-4 text-left text-[13px] text-(--foreground)",
        indent ? "pl-[34px]" : "pl-[18px]",
        selected
          ? "bg-(--background-accent-stronger) font-medium"
          : "hover:bg-(--background-accent)"
      )}
    >
      <span className="flex w-[18px] justify-center">
        {Icon && (
          <Icon
            className="size-4 text-(--muted-foreground)"
            strokeWidth={1.75}
          />
        )}
        {dot && (
          <span className="size-2 rounded-full" style={{ background: dot }} />
        )}
      </span>
      <span className="flex-1 truncate">{title}</span>
      {count > 0 && (
        <span className="text-[12px] text-(--muted-stronger-foreground)">
          {count}
        </span>
      )}
      {children}
    </button>
  )
}

/** "Snooze until": the usual times, narrowed by what is typed. */
export function SnoozeSheet({
  now,
  onPick,
  onClose,
}: {
  now: Date
  onPick: (at: Date) => void
  onClose: () => void
}) {
  const [query, setQuery] = useState("")
  const choices = snoozeTimes(now).filter((choice) =>
    choice.name.toLowerCase().includes(query.trim().toLowerCase())
  )
  const [chosen, setChosen] = useState(0)

  return (
    <Sheet title="Snooze until" onClose={onClose}>
      <label className="flex h-11 items-center gap-2 border-b border-(--border-popover) px-3">
        <ClockIcon
          className="size-3.5 text-(--muted-stronger-foreground)"
          strokeWidth={1.75}
        />
        <input
          autoFocus
          value={query}
          placeholder="Later today, tomorrow, next week"
          onChange={(event) => {
            setQuery(event.target.value)
            setChosen(0)
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault()
              const step = event.key === "ArrowDown" ? 1 : -1
              setChosen(
                Math.max(0, Math.min(choices.length - 1, chosen + step))
              )
            }
            if (event.key === "Enter" && choices[chosen]) {
              onPick(choices[chosen].at)
            }
          }}
          className="min-w-0 flex-1 bg-transparent text-[15px] text-(--foreground) outline-none placeholder:text-(--muted-stronger-foreground)"
        />
      </label>
      <div className="min-h-[240px] p-2">
        {choices.map((choice, index) => (
          <button
            key={choice.name}
            type="button"
            onMouseEnter={() => setChosen(index)}
            onClick={() => onPick(choice.at)}
            className={cn(
              "flex h-9 w-full items-center justify-between rounded-sm px-2.5 text-[14px] text-(--foreground)",
              index === chosen && "bg-(--popover-accent)"
            )}
          >
            {choice.name}
            <span className="text-[12px] text-(--muted-stronger-foreground)">
              {snoozeDate(choice.at, now)}
            </span>
          </button>
        ))}
      </div>
    </Sheet>
  )
}
