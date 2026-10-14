import {
  AlarmClockIcon,
  ArrowLeftIcon,
  CalendarClockIcon,
  ChevronDownIcon,
  PaperclipIcon,
  SendIcon,
  Trash2Icon,
  ZapIcon,
} from "lucide-react"
import { useEffect, useRef, type ReactNode } from "react"
import { ACCOUNTS } from "./mail"
import { ActionButton, IconButton } from "./ui"

export interface Draft {
  account: number
  to: string
  subject: string
  text: string
  /** The thread it answers, when it is a reply. */
  replyTo?: string
}

/** Writing, a page of its own as in the Mac app: back, the title and Send over a card. */
export function Compose({
  draft,
  onChange,
  onSend,
  onClose,
  onDiscard,
  onNotInDemo,
}: {
  draft: Draft
  onChange: (draft: Draft) => void
  onSend: () => void
  onClose: () => void
  onDiscard: () => void
  onNotInDemo: () => void
}) {
  const to = useRef<HTMLInputElement>(null)
  const body = useRef<HTMLTextAreaElement>(null)
  const account = ACCOUNTS[draft.account]

  // Where the cursor starts, once.
  useEffect(() => {
    if (draft.to) body.current?.focus()
    else to.current?.focus()
  }, [])

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="relative flex h-[52px] shrink-0 items-center border-b border-(--border) pr-5 pl-[99px]">
        <IconButton icon={ArrowLeftIcon} label="Back (Esc)" onClick={onClose} />
        <span className="absolute left-1/2 -translate-x-1/2 text-[14px] font-semibold text-(--foreground)">
          {draft.replyTo ? draft.subject : "New message"}
        </span>
        <ActionButton
          icon={SendIcon}
          title="Send"
          variant="primary"
          className="ml-auto"
          disabled={!draft.to.trim()}
          onClick={onSend}
        />
      </div>
      <div className="mx-auto mt-4 flex min-h-0 w-[1000px] flex-1 flex-col rounded-sm border border-(--border-card) bg-(--card-larger) px-20">
        <Field label="To">
          <input
            ref={to}
            value={draft.to}
            onChange={(event) => onChange({ ...draft, to: event.target.value })}
            className="min-w-0 flex-1 bg-transparent text-(--foreground) outline-none"
          />
          <span className="text-[13px] text-(--muted-foreground)">Cc Bcc</span>
        </Field>
        <Field label="From">
          <span
            className="size-[7px] rounded-full"
            style={{ background: `var(--${account.color})` }}
          />
          <span className="font-medium text-(--foreground)">Sam Lee</span>
          <span className="text-[12px] text-(--muted-stronger-foreground)">
            {account.email}
          </span>
          <ChevronDownIcon
            className="size-3 text-(--muted-stronger-foreground)"
            strokeWidth={2}
          />
        </Field>
        <Field label="Subject">
          <input
            value={draft.subject}
            onChange={(event) =>
              onChange({ ...draft, subject: event.target.value })
            }
            className="min-w-0 flex-1 bg-transparent text-(--foreground) outline-none"
          />
        </Field>
        <textarea
          ref={body}
          value={draft.text}
          onChange={(event) => onChange({ ...draft, text: event.target.value })}
          className="min-h-0 flex-1 resize-none bg-transparent pt-5 text-[15px] leading-[1.55] text-(--foreground) outline-none"
        />
      </div>
      <div className="flex h-[52px] shrink-0 items-center gap-1.5 border-t border-(--border) px-5">
        <IconButton icon={PaperclipIcon} label="Attach" onClick={onNotInDemo} />
        <IconButton icon={ZapIcon} label="Snippets" onClick={onNotInDemo} />
        <IconButton
          icon={CalendarClockIcon}
          label="Send later"
          onClick={onNotInDemo}
        />
        <IconButton
          icon={AlarmClockIcon}
          label="Remind me"
          onClick={onNotInDemo}
        />
        <IconButton
          icon={Trash2Icon}
          label="Discard"
          className="ml-auto"
          onClick={onDiscard}
        />
      </div>
    </div>
  )
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex h-[45px] shrink-0 items-center gap-2 border-b border-(--border-card) text-[14px]">
      <span className="w-[58px] shrink-0 text-[13px] text-(--muted-stronger-foreground)">
        {label}
      </span>
      {children}
    </label>
  )
}
