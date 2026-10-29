import {
  ForwardIcon,
  PaperclipIcon,
  ReplyAllIcon,
  ReplyIcon,
  StarIcon,
  TagIcon,
} from "lucide-react"
import { useState } from "react"
import { ACCOUNTS, avatarColor, initials, longDate } from "./mail"
import type { Message, Thread } from "./mail"
import { AttachmentCard, Avatar, Chip, IconButton, cn } from "./ui"

export type Reply = "reply" | "replyAll" | "forward"

/** The open thread on its card: subject, who it is between, its labels, the messages (the last
 * one open) and the reply buttons. */
export function ThreadView({
  thread,
  now,
  onStar,
  onReply,
  onClose,
}: {
  thread: Thread
  now: Date | null
  onStar: () => void
  onReply: (kind: Reply) => void
  onClose: () => void
}) {
  const me = ACCOUNTS[thread.account].email
  const [opened, setOpened] = useState(
    () => new Set([thread.messages.length - 1])
  )

  return (
    <div
      className="absolute inset-0 overflow-y-auto"
      onClick={(event) => {
        // The page around the card: a click there closes the thread, as in the Mac app.
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <article className="mx-auto mt-4 mb-6 w-[1000px] rounded-sm border border-(--border) bg-(--card-larger) px-20 py-7">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <h2 className="text-[24px] leading-tight font-semibold text-(--foreground)">
              {thread.subject}
            </h2>
            <p className="mt-1.5 text-[12px] text-(--muted-foreground)">
              {thread.with.name}{" "}
              <span className="text-(--muted-stronger-foreground)">to</span>{" "}
              {me}
            </p>
          </div>
          <button
            type="button"
            onClick={onStar}
            aria-label={thread.starred ? "Unstar" : "Star"}
            title={thread.starred ? "Unstar" : "Star"}
            className={cn(
              "flex size-7 items-center justify-center rounded-full hover:text-(--star)",
              thread.starred
                ? "text-(--star)"
                : "text-(--muted-stronger-foreground)"
            )}
          >
            <StarIcon
              className="size-[18px]"
              strokeWidth={1.75}
              fill={thread.starred ? "currentColor" : "none"}
            />
          </button>
        </div>
        {thread.label && (
          <div className="mt-3">
            <Chip icon={TagIcon}>{thread.label}</Chip>
          </div>
        )}
        <div className="mt-5">
          {thread.messages.map((message, index) => (
            <MessageView
              key={index}
              thread={thread}
              message={message}
              me={me}
              now={now}
              open={opened.has(index)}
              first={index === 0}
              attachment={
                !message.mine &&
                (index === 0 || index === thread.messages.length - 1)
                  ? thread.attachment
                  : undefined
              }
              onOpen={() =>
                setOpened((all) => {
                  const next = new Set(all)
                  if (next.has(index)) next.delete(index)
                  else next.add(index)
                  return next
                })
              }
            />
          ))}
        </div>
        <div className="mt-[22px] flex gap-1.5">
          <IconButton
            icon={ReplyAllIcon}
            label="Reply all (A)"
            onClick={() => onReply("replyAll")}
          />
          <IconButton
            icon={ReplyIcon}
            label="Reply (R)"
            onClick={() => onReply("reply")}
          />
          <IconButton
            icon={ForwardIcon}
            label="Forward (F)"
            onClick={() => onReply("forward")}
          />
        </div>
      </article>
    </div>
  )
}

function MessageView({
  thread,
  message,
  me,
  now,
  open,
  first,
  attachment,
  onOpen,
}: {
  thread: Thread
  message: Message
  me: string
  now: Date | null
  open: boolean
  first: boolean
  attachment?: string
  onOpen: () => void
}) {
  const from = message.mine ? { name: "me", email: me } : thread.with
  const to = message.mine ? thread.with.email : me
  const date =
    now && longDate(new Date(now.getTime() - message.ago * 60_000), now)

  return (
    <section
      className={cn("py-4", !first && "border-t border-(--border-card)")}
    >
      <div className="flex cursor-default items-center gap-3" onClick={onOpen}>
        <Avatar
          initials={message.mine ? "S" : initials(from.name)}
          color={avatarColor(from.email)}
        />
        <div className="min-w-0 flex-1">
          <p
            className={cn(
              "text-[14px]",
              open
                ? "font-semibold text-(--foreground)"
                : "text-(--muted-foreground)"
            )}
          >
            {message.mine ? "me" : from.name}
          </p>
          <p className="truncate text-[12px] text-(--muted-stronger-foreground)">
            {open ? `to ${to}` : message.text}
          </p>
        </div>
        {attachment && (
          <PaperclipIcon
            className="size-[13px] text-(--muted-stronger-foreground)"
            strokeWidth={1.75}
          />
        )}
        <span className="shrink-0 text-[12px] text-(--muted-stronger-foreground)">
          {date}
        </span>
      </div>
      {open && (
        <div className="pl-[46px]">
          <p className="mt-3 text-[15px] leading-[1.55] text-(--foreground)">
            {message.text}
          </p>
          <p className="mt-6 text-[15px] text-(--foreground)">
            {message.mine ? "Sam" : thread.with.name.split(" ")[0]}
          </p>
          {attachment && (
            <div className="mt-[18px]">
              <p className="mb-2 text-[12px] text-(--muted-stronger-foreground)">
                1 attachment
              </p>
              <AttachmentCard name={attachment} size="184 KB" />
            </div>
          )}
        </div>
      )}
    </section>
  )
}
