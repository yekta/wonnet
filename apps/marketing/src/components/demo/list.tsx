import {
  ArchiveIcon,
  ClockIcon,
  InboxIcon,
  MailIcon,
  MailOpenIcon,
  PaperclipIcon,
  SearchIcon,
  SendIcon,
  StarIcon,
  Trash2Icon,
  type LucideIcon,
} from "lucide-react"
import type { Ref } from "react"
import { ACCOUNTS, TABS, latest, senders, shortDate } from "./mail"
import type { Mailbox, Thread } from "./mail"
import { PillTab, SelectCircle, cn } from "./ui"

const TAB_ICONS: Partial<Record<Mailbox, LucideIcon>> = {
  inbox: InboxIcon,
  unread: MailIcon,
  sent: SendIcon,
  snoozed: ClockIcon,
}

export function MailboxTabs({
  mailbox,
  unread,
  onPick,
}: {
  mailbox: Mailbox
  unread: number
  onPick: (mailbox: Mailbox) => void
}) {
  return (
    <div className="flex gap-1">
      {TABS.map((tab) => (
        <PillTab
          key={tab.id}
          icon={TAB_ICONS[tab.id]!}
          title={tab.name}
          count={tab.id === "inbox" || tab.id === "unread" ? unread : 0}
          chosen={mailbox === tab.id}
          onClick={() => onPick(tab.id)}
        />
      ))}
    </div>
  )
}

export function SearchField({
  query,
  inputRef,
  onChange,
}: {
  query: string
  inputRef: Ref<HTMLInputElement>
  onChange: (query: string) => void
}) {
  return (
    <label className="flex h-9 w-40 items-center gap-2 rounded-full border border-(--border-input) bg-(--input) px-3 text-[13px] text-(--foreground)">
      <SearchIcon
        className="size-3.5 shrink-0 text-(--muted-stronger-foreground)"
        strokeWidth={1.75}
      />
      <input
        ref={inputRef}
        value={query}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Search"
        className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-(--muted-stronger-foreground)"
      />
    </label>
  )
}

export type RowAction = "archive" | "trash" | "snooze" | "read" | "star"

/** One thread on one line, as the Mac's table draws it: the account's bar, unread dot, senders,
 * subject and snippet, the star if starred, date; hovered, what can be done to it over the date. In the
 * gutter before it, the circle that selects it; while threads are selected, a click selects. */
export function ThreadRow({
  thread,
  current,
  checked,
  selecting,
  now,
  onOpen,
  onSelect,
  onAction,
}: {
  thread: Thread
  current: boolean
  checked: boolean
  selecting: boolean
  now: Date | null
  onOpen: () => void
  onSelect: (range: boolean) => void
  onAction: (action: RowAction) => void
}) {
  const message = latest(thread)
  const strong = thread.unread
  const actions: [RowAction, LucideIcon, string][] = [
    ["archive", ArchiveIcon, "Archive"],
    ["trash", Trash2Icon, "Trash"],
    ["snooze", ClockIcon, "Snooze"],
    [
      "read",
      thread.unread ? MailOpenIcon : MailIcon,
      thread.unread ? "Mark read" : "Mark unread",
    ],
    ["star", StarIcon, thread.starred ? "Unstar" : "Star"],
  ]

  return (
    <div className="group flex">
      <SelectCircle
        checked={checked}
        label={checked ? "Deselect" : "Select (X)"}
        onClick={(event) => onSelect(event.shiftKey)}
        className={cn(
          "h-12 w-9",
          !checked && !selecting && "invisible group-hover:visible"
        )}
      />
      <div className="min-w-0 flex-1 py-px">
        <div
          onClick={(event) =>
            selecting || event.metaKey || event.shiftKey
              ? onSelect(event.shiftKey)
              : onOpen()
          }
          className={cn(
            "relative flex h-[46px] cursor-default items-center rounded-sm text-[13px] group-hover:bg-(--background-accent-larger)",
            current &&
              "outline-1 -outline-offset-1 outline-(--background-accent-stronger) outline-dashed"
          )}
        >
          {checked && (
            <span className="pointer-events-none absolute inset-0 rounded-sm bg-[color-mix(in_srgb,var(--primary)_calc(var(--opacity-color-tint)*100%),transparent)]" />
          )}
          <span
            className="absolute top-[5px] bottom-[5px] left-[3px] w-0.5 rounded-full"
            style={{ background: `var(--${ACCOUNTS[thread.account].color})` }}
          />
          {thread.unread && (
            <span className="absolute left-[18px] size-[7px] rounded-full bg-(--primary)" />
          )}
          <span
            className={cn(
              "ml-9 w-[190px] shrink-0 truncate",
              strong
                ? "font-bold text-(--foreground)"
                : "text-(--muted-foreground)"
            )}
          >
            {senders(thread)}
          </span>
          <span className="ml-3.5 flex w-6 shrink-0">
            {thread.attachment && (
              <PaperclipIcon
                className="size-3.5 text-(--muted-stronger-foreground)"
                strokeWidth={1.75}
              />
            )}
          </span>
          <span
            className={cn(
              "min-w-0 flex-1 truncate",
              thread.starred ? "pr-2" : "pr-4"
            )}
          >
            <span
              className={cn(
                strong
                  ? "font-semibold text-(--foreground)"
                  : "text-(--muted-foreground)"
              )}
            >
              {thread.subject}
            </span>
            <span className="whitespace-pre text-(--muted-stronger-foreground)">
              {"  –  "}
              {message.text}
            </span>
          </span>
          {thread.starred && (
            <StarIcon
              className={cn(
                "mr-3 size-3.5 shrink-0 text-(--star)",
                !selecting && "group-hover:invisible"
              )}
              strokeWidth={1.75}
              fill="currentColor"
            />
          )}
          <span
            className={cn(
              "mr-4 shrink-0 text-[12px]",
              !selecting && "group-hover:invisible",
              strong
                ? "font-semibold text-(--foreground)"
                : "text-(--muted-stronger-foreground)"
            )}
          >
            {now &&
              shortDate(new Date(now.getTime() - message.ago * 60_000), now)}
          </span>
          <div
            className={cn(
              "absolute right-2.5 hidden items-center gap-0.5 bg-(--background-accent-larger) pl-2",
              !selecting && "group-hover:flex"
            )}
          >
            <span className="absolute top-0 right-full h-full w-8 bg-linear-to-r from-transparent to-(--background-accent-larger)" />
            {actions.map(([action, Icon, label]) => (
              <button
                key={action}
                type="button"
                title={label}
                aria-label={label}
                onClick={(event) => {
                  event.stopPropagation()
                  onAction(action)
                }}
                className={cn(
                  "flex size-7 items-center justify-center rounded-full",
                  action === "star"
                    ? "hover:bg-[color-mix(in_srgb,var(--star)_calc(var(--opacity-color-tint)*100%),transparent)] hover:text-(--star)"
                    : "hover:bg-(--background-accent-larger-strongest) hover:text-(--foreground)",
                  action === "star" && thread.starred
                    ? "text-(--star)"
                    : "text-(--muted-foreground)"
                )}
              >
                <Icon
                  className="size-4"
                  strokeWidth={1.75}
                  fill={
                    action === "star" && thread.starred
                      ? "currentColor"
                      : "none"
                  }
                />
              </button>
            ))}
          </div>
        </div>
      </div>
      <div className="pointer-events-none w-9 shrink-0" />
    </div>
  )
}

export function EmptyList({ message }: { message: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 pb-24 text-[14px] text-(--muted-foreground)">
      <InboxIcon
        className="size-7 text-(--muted-stronger-foreground)"
        strokeWidth={1.5}
      />
      {message}
    </div>
  )
}
