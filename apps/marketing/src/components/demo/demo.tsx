import {
  ArchiveIcon,
  ArrowLeftIcon,
  ClockIcon,
  EllipsisIcon,
  MailIcon,
  MailOpenIcon,
  MenuIcon,
  SquarePenIcon,
  TagIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react"
import { useEffect, useRef, useState, type KeyboardEvent } from "react"
import { Compose, type Draft } from "./compose"
import { EmptyList, MailboxTabs, SearchField, ThreadRow } from "./list"
import type { RowAction } from "./list"
import {
  ACCOUNTS,
  MAILBOXES,
  THREADS,
  inMailbox,
  latest,
  snoozeDate,
} from "./mail"
import type { Box, Mailbox, Thread } from "./mail"
import { Sidebar, SnoozeSheet } from "./sidebar"
import { ThreadView, type Reply } from "./thread"
import {
  ActionButton,
  IconButton,
  SelectCircle,
  Toast,
  TrafficLights,
  shadow,
} from "./ui"

interface View {
  mailbox: Mailbox
  account: number | null
}

interface Notice {
  message: string
  /** The threads as they were, put back by Undo. */
  undo?: Thread[]
  /** The draft that was sent, opened again by Undo. */
  reopen?: Draft
}

const FONT = '"Avenir Next", Avenir, "DM Sans Variable", sans-serif'

/** The Mac app's window with the demo's mail, which can be read, archived, snoozed and answered. */
export function Demo() {
  const [threads, setThreads] = useState(THREADS)
  const [view, setView] = useState<View>({
    mailbox: "inbox",
    account: null,
  })
  const [openId, setOpenId] = useState<string | null>(null)
  const [cursor, setCursor] = useState(0)
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [snoozing, setSnoozing] = useState<string[] | null>(null)
  const [selection, setSelection] = useState<string[]>([])
  /** Where a Shift-click's range starts. */
  const [anchor, setAnchor] = useState<string | null>(null)
  const [query, setQuery] = useState("")
  const [notice, setNotice] = useState<Notice | null>(null)
  const now = useNow()
  const search = useRef<HTMLInputElement>(null)
  const frame = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!notice) return
    const timer = setTimeout(() => setNotice(null), 5000)
    return () => clearTimeout(timer)
  }, [notice])

  const ofAccount = (thread: Thread, account: number | null) =>
    account === null || thread.account === account
  const searching = query.trim().toLowerCase()
  const rows = threads
    .filter((thread) =>
      searching
        ? thread.box !== "trash" &&
          [
            thread.subject,
            thread.with.name,
            ...thread.messages.map((one) => one.text),
          ]
            .join(" ")
            .toLowerCase()
            .includes(searching)
        : ofAccount(thread, view.account) && inMailbox(thread, view.mailbox)
    )
    .sort((a, b) => latest(a).ago - latest(b).ago)
  const unreadIn = (mailbox: Mailbox, account: number | null) =>
    threads.filter(
      (thread) =>
        ofAccount(thread, account) &&
        inMailbox(thread, mailbox) &&
        (mailbox === "starred" ? true : thread.unread)
    ).length
  const sidebarCount = (mailbox: Mailbox, account: number | null) =>
    mailbox === "inbox" || mailbox === "starred"
      ? unreadIn(mailbox, account)
      : 0
  const open = threads.find((thread) => thread.id === openId) ?? null
  const current = Math.min(cursor, Math.max(rows.length - 1, 0))
  const target = open?.id ?? rows[current]?.id
  const chosen = open
    ? []
    : rows
        .filter((thread) => selection.includes(thread.id))
        .map((one) => one.id)
  /** The threads an action is for: the selection over the list, else the current thread. */
  const targets = chosen.length > 0 ? chosen : target ? [target] : []

  const change = (ids: string[], edit: (thread: Thread) => Partial<Thread>) =>
    setThreads((all) =>
      all.map((thread) =>
        ids.includes(thread.id) ? { ...thread, ...edit(thread) } : thread
      )
    )

  /** Moves the threads out of where they are, with Undo, and back to the list if one was open. */
  const move = (ids: string[], box: Box, message: string, until?: number) => {
    setNotice({ message, undo: threads })
    change(ids, () => ({ box, snoozedUntil: until, unread: false }))
    setSelection((all) => all.filter((id) => !ids.includes(id)))
    if (openId && ids.includes(openId)) setOpenId(null)
  }

  const of = (ids: string[]) => threads.filter((one) => ids.includes(one.id))

  const act = (action: RowAction, ids: string[] = targets) => {
    if (ids.length === 0) return
    switch (action) {
      case "archive":
        return move(ids, "archive", "Archived")
      case "trash":
        return move(ids, "trash", "Moved to Trash")
      case "snooze":
        return setSnoozing(ids)
      case "star": {
        const starred = of(ids).every((one) => one.starred)
        return change(ids, () => ({ starred: !starred }))
      }
      case "read": {
        const unread = of(ids).some((one) => one.unread)
        if (openId && ids.includes(openId)) setOpenId(null)
        return change(ids, () => ({ unread: !unread }))
      }
    }
  }

  /** Selects the thread or leaves it out; with Shift, selects the rows from the last one picked. */
  const select = (id: string, range = false) => {
    const index = rows.findIndex((thread) => thread.id === id)
    setCursor(index)
    const from = rows.findIndex((thread) => thread.id === anchor)
    if (range && from >= 0) {
      const span = rows
        .slice(Math.min(from, index), Math.max(from, index) + 1)
        .map((thread) => thread.id)
      return setSelection((all) => [...new Set([...all, ...span])])
    }
    setAnchor(id)
    setSelection((all) =>
      all.includes(id) ? all.filter((one) => one !== id) : [...all, id]
    )
  }

  const allChosen = rows.length > 0 && chosen.length === rows.length
  const clearSelection = () => setSelection([])
  const selectAll = () => setSelection(rows.map((thread) => thread.id))

  const openThread = (id: string) => {
    setOpenId(id)
    setCursor(rows.findIndex((thread) => thread.id === id))
    change([id], () => ({ unread: false }))
  }

  const step = (by: number) => {
    const index = Math.max(0, Math.min(rows.length - 1, current + by))
    if (open && rows[index]) openThread(rows[index].id)
    else setCursor(index)
  }

  const pick = (mailbox: Mailbox, account: number | null = view.account) => {
    setView({ ...view, mailbox, account })
    setOpenId(null)
    setQuery("")
    setCursor(0)
    setSelection([])
    setSidebarOpen(false)
  }

  const reply = (kind: Reply, id: string | undefined = target) => {
    const thread = threads.find((one) => one.id === id)
    if (!thread) return
    const forward = kind === "forward"
    setDraft({
      account: thread.account,
      to: forward ? "" : thread.with.name,
      subject: `${forward ? "Fwd" : "Re"}: ${thread.subject}`,
      text: "",
      replyTo: forward ? undefined : thread.id,
    })
  }

  const send = () => {
    if (!draft) return
    const ago = -(Date.now() - (now?.getTime() ?? Date.now())) / 60_000
    const text = draft.text.trim() || "(no message)"
    setNotice({ message: "Sent", undo: threads, reopen: draft })
    if (draft.replyTo) {
      change([draft.replyTo], (thread) => ({
        messages: [...thread.messages, { mine: true, text, ago }],
      }))
    } else {
      const name = draft.to.trim()
      setThreads((all) => [
        ...all,
        {
          id: `sent-${all.length}`,
          account: draft.account,
          with: { name, email: name.includes("@") ? name : "" },
          subject: draft.subject.trim() || "(no subject)",
          messages: [{ mine: true, text, ago }],
          box: "archive",
          unread: false,
          starred: false,
        },
      ])
    }
    closeDraft()
  }

  const undo = () => {
    if (!notice?.undo) return
    setThreads(notice.undo)
    if (notice.reopen) setDraft(notice.reopen)
    setNotice(null)
  }

  /** Gives the keys back to the window, as a sheet or the composer leaves. */
  const refocus = () => frame.current?.focus({ preventScroll: true })
  const closeDraft = () => {
    refocus()
    setDraft(null)
  }

  const notInDemo = () =>
    setNotice({ message: "That one is in the app, not the demo." })

  const escape = () => {
    if (snoozing) {
      refocus()
      setSnoozing(null)
    } else if (draft) closeDraft()
    else if (sidebarOpen) setSidebarOpen(false)
    else if (document.activeElement === search.current) {
      setQuery("")
      search.current?.blur()
    } else if (!open && chosen.length > 0) clearSelection()
    else setOpenId(null)
  }

  const keys: Record<string, () => void> = {
    j: () => step(1),
    ArrowDown: () => step(1),
    k: () => step(-1),
    ArrowUp: () => step(-1),
    Enter: () => target && openThread(target),
    o: () => target && openThread(target),
    e: () => act("archive"),
    "#": () => act("trash"),
    Backspace: () => act("trash"),
    s: () => act("star"),
    u: () => act("read"),
    h: () => act("snooze"),
    c: () => setDraft({ account: 0, to: "", subject: "", text: "" }),
    r: () => reply("reply"),
    a: () => reply("replyAll"),
    f: () => reply("forward"),
    x: () => !open && rows[current] && select(rows[current].id),
    z: undo,
    "/": () => search.current?.focus(),
  }

  const onKeyDown = (event: KeyboardEvent) => {
    const typing = (event.target as HTMLElement).closest("input, textarea")
    if (event.metaKey && event.key === "a" && !typing && !open && !draft) {
      event.preventDefault()
      return selectAll()
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (event.key === "Escape") {
      event.preventDefault()
      return escape()
    }
    if (typing || draft || snoozing) return
    const run = keys[event.key]
    if (!run) return
    event.preventDefault()
    run()
  }

  const title = (() => {
    const name =
      view.mailbox === "inbox" && view.account === null
        ? "All Inboxes"
        : (MAILBOXES.find((one) => one.id === view.mailbox)?.name ?? "Unread")
    return view.account === null
      ? name
      : `${name} · ${ACCOUNTS[view.account].name}`
  })()

  const empty = searching
    ? "Nothing matches."
    : view.mailbox === "unread"
      ? "Nothing unread."
      : view.mailbox === "inbox"
        ? "All done. Enjoy the quiet."
        : "Nothing here."

  return (
    <div className="@container w-full">
      <div className="h-[calc(760px*var(--demo-scale))] [--demo-scale:var(--demo-fit,min(1,tan(atan2(100cqw,1200px))))]">
        <div
          ref={frame}
          tabIndex={-1}
          onKeyDown={onKeyDown}
          onMouseDown={(event) => {
            if (!(event.target as HTMLElement).closest("input, textarea"))
              event.currentTarget.focus({ preventScroll: true })
          }}
          className="relative flex h-[760px] w-[1200px] origin-top-left scale-(--demo-scale) flex-col overflow-hidden rounded-lg bg-(--background) text-left text-(--foreground) outline-none select-none"
          style={{
            fontFamily: FONT,
            boxShadow: `0 0 0 1px var(--color-border), ${shadow("xl", "shadow-stronger")}`,
          }}
        >
          <div className="absolute top-5 left-5 z-30">
            <TrafficLights />
          </div>
          {!draft && (
            <div className="absolute top-2 left-[99px] z-30 flex h-9 items-center gap-1.5">
              <IconButton
                icon={MenuIcon}
                label="Mailboxes"
                circled={false}
                onClick={() => setSidebarOpen(!sidebarOpen)}
              />
              {open && !sidebarOpen && (
                <IconButton
                  icon={ArrowLeftIcon}
                  label="Back (Esc)"
                  onClick={() => setOpenId(null)}
                />
              )}
            </div>
          )}

          {draft ? (
            <Compose
              draft={draft}
              onChange={setDraft}
              onSend={send}
              onClose={closeDraft}
              onDiscard={closeDraft}
              onNotInDemo={notInDemo}
            />
          ) : (
            <>
              {!open && chosen.length > 0 ? (
                <div className="flex h-[52px] shrink-0 items-center border-b border-(--border) pr-5 pl-[134px]">
                  <SelectCircle
                    checked={allChosen}
                    label={allChosen ? "Select none" : "Select all (⌘A)"}
                    onClick={allChosen ? clearSelection : selectAll}
                  />
                  <span className="text-[13px] font-semibold text-(--foreground)">
                    {chosen.length} selected
                  </span>
                  <div className="ml-auto flex items-center gap-0.5">
                    <ActionButton
                      variant="ghost"
                      icon={ArchiveIcon}
                      title="Archive"
                      onClick={() => act("archive")}
                    />
                    <ActionButton
                      variant="ghost"
                      icon={Trash2Icon}
                      title="Trash"
                      onClick={() => act("trash")}
                    />
                    <ActionButton
                      variant="ghost"
                      icon={ClockIcon}
                      title="Snooze"
                      onClick={() => act("snooze")}
                    />
                    <ActionButton
                      variant="ghost"
                      icon={TagIcon}
                      title="Label"
                      onClick={notInDemo}
                    />
                    <ActionButton
                      variant="ghost"
                      icon={MailOpenIcon}
                      title={
                        of(chosen).some((one) => one.unread)
                          ? "Mark read"
                          : "Mark unread"
                      }
                      onClick={() => act("read")}
                    />
                    <IconButton
                      icon={EllipsisIcon}
                      label="More"
                      circled={false}
                      onClick={notInDemo}
                    />
                    <IconButton
                      icon={XIcon}
                      label="Clear the selection (Esc)"
                      circled={false}
                      onClick={clearSelection}
                    />
                  </div>
                </div>
              ) : (
                <div className="relative flex h-[52px] shrink-0 items-center border-b border-(--border) pr-5 pl-[141px]">
                  {!open && (
                    <span className="text-[14px] font-semibold text-(--foreground)">
                      {title}
                    </span>
                  )}
                  <div className="absolute left-1/2 -translate-x-1/2">
                    {open ? (
                      <div className="flex gap-1.5">
                        <IconButton
                          icon={ArchiveIcon}
                          label="Archive (E)"
                          onClick={() => act("archive")}
                        />
                        <IconButton
                          icon={Trash2Icon}
                          label="Trash (#)"
                          onClick={() => act("trash")}
                        />
                        <IconButton
                          icon={ClockIcon}
                          label="Snooze (H)"
                          onClick={() => act("snooze")}
                        />
                        <IconButton
                          icon={TagIcon}
                          label="Label (L)"
                          onClick={notInDemo}
                        />
                        <IconButton
                          icon={MailIcon}
                          label="Mark unread (U)"
                          onClick={() => act("read")}
                        />
                        <IconButton
                          icon={EllipsisIcon}
                          label="More"
                          onClick={notInDemo}
                        />
                      </div>
                    ) : (
                      <MailboxTabs
                        mailbox={view.mailbox}
                        unread={unreadIn("inbox", view.account)}
                        onPick={(mailbox) => pick(mailbox)}
                      />
                    )}
                  </div>
                  <div className="ml-auto flex items-center gap-1.5">
                    {!open && (
                      <SearchField
                        query={query}
                        inputRef={search}
                        onChange={(text) => {
                          setQuery(text)
                          setCursor(0)
                        }}
                      />
                    )}
                    <ActionButton
                      icon={SquarePenIcon}
                      title="Compose"
                      onClick={() =>
                        setDraft({ account: 0, to: "", subject: "", text: "" })
                      }
                    />
                  </div>
                </div>
              )}

              <div className="relative min-h-0 flex-1">
                {open ? (
                  <ThreadView
                    key={open.id}
                    thread={open}
                    now={now}
                    onStar={() => act("star", [open.id])}
                    onReply={(kind) => reply(kind, open.id)}
                    onClose={() => setOpenId(null)}
                  />
                ) : (
                  <div className="absolute inset-0 flex flex-col overflow-y-auto py-4">
                    {rows.length === 0 ? (
                      <EmptyList message={empty} />
                    ) : (
                      <div className="mx-auto w-[1072px] shrink-0">
                        {rows.map((thread, index) => (
                          <ThreadRow
                            key={thread.id}
                            thread={thread}
                            now={now}
                            current={index === current}
                            checked={chosen.includes(thread.id)}
                            selecting={chosen.length > 0}
                            onOpen={() => openThread(thread.id)}
                            onSelect={(range) => select(thread.id, range)}
                            onAction={(action) => {
                              setCursor(index)
                              act(
                                action,
                                chosen.includes(thread.id)
                                  ? chosen
                                  : [thread.id]
                              )
                            }}
                          />
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </>
          )}

          <Sidebar
            open={sidebarOpen}
            mailbox={view.mailbox}
            account={view.account}
            count={sidebarCount}
            onPick={pick}
            onClose={() => setSidebarOpen(false)}
            onSettings={() => {
              setSidebarOpen(false)
              notInDemo()
            }}
          />

          {snoozing && now && (
            <SnoozeSheet
              now={now}
              onClose={() => {
                refocus()
                setSnoozing(null)
              }}
              onPick={(at) => {
                refocus()
                setSnoozing(null)
                move(
                  snoozing,
                  "snoozed",
                  `Snoozed until ${snoozeDate(at, now)}`,
                  at.getTime()
                )
              }}
            />
          )}

          {notice && (
            <Toast
              message={notice.message}
              action={notice.undo ? "Undo" : undefined}
              onAction={undo}
            />
          )}
        </div>
      </div>
    </div>
  )
}

/** The time the page opened, once it has: the server's would date the mail in its own zone. */
function useNow() {
  const [now, setNow] = useState<Date | null>(null)
  useEffect(() => setNow(new Date()), [])
  return now
}
