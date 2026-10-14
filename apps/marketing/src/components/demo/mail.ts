/** The core's made-up mail (crates/core/src/demo.rs), for the site's demo of the Mac app. */

export type Box = "inbox" | "archive" | "snoozed" | "trash"
export type Mailbox =
  | "inbox"
  | "unread"
  | "starred"
  | "snoozed"
  | "sent"
  | "drafts"
  | "archive"
  | "spam"
  | "trash"

export interface Person {
  name: string
  email: string
}

export interface Message {
  mine: boolean
  text: string
  /** Minutes before the page opened. */
  ago: number
}

export interface Thread {
  id: string
  account: number
  with: Person
  subject: string
  label?: string
  messages: Message[]
  box: Box
  unread: boolean
  starred: boolean
  attachment?: string
  snoozedUntil?: number
}

export const ACCOUNTS = [
  { name: "sam@example.com", email: "sam@example.com", color: "account-2" },
  { name: "Acme", email: "sam@acme.example", color: "account-5" },
]

export const MAILBOXES: { id: Mailbox; name: string }[] = [
  { id: "inbox", name: "Inbox" },
  { id: "starred", name: "Starred" },
  { id: "snoozed", name: "Snoozed" },
  { id: "sent", name: "Sent" },
  { id: "drafts", name: "Drafts" },
  { id: "archive", name: "Archive" },
  { id: "spam", name: "Spam" },
  { id: "trash", name: "Trash" },
]

export const TABS: { id: Mailbox; name: string }[] = [
  { id: "inbox", name: "Inbox" },
  { id: "unread", name: "Unread" },
  { id: "sent", name: "Sent" },
  { id: "snoozed", name: "Snoozed" },
]

const HOUR = 60
const DAY = 24 * HOUR

let next = 0
function thread(
  fields: Partial<Thread> & Pick<Thread, "with" | "subject" | "messages">
): Thread {
  next += 1
  return {
    id: `thread-${next}`,
    account: 0,
    box: "inbox",
    unread: false,
    starred: false,
    ...fields,
  }
}

const said = (text: string, ago: number): Message => ({
  mine: false,
  text,
  ago,
})
const wrote = (text: string, ago: number): Message => ({
  mine: true,
  text,
  ago,
})

export const THREADS: Thread[] = [
  thread({
    account: 1,
    with: { name: "Priya Shah", email: "priya@acme.example" },
    subject: "Launch checklist for Thursday",
    messages: [
      said(
        "Here is the checklist for Thursday. Can you take the release notes and the status page?",
        3 * HOUR
      ),
      wrote(
        "Yes, both are mine. I'll have the notes in the doc by tomorrow noon.",
        2 * HOUR
      ),
      said(
        "Perfect. I moved the go/no-go to 9:30 so we have time to fix anything that comes up.",
        12
      ),
    ],
    unread: true,
    attachment: "Launch checklist.pdf",
  }),
  thread({
    with: { name: "The Morning Orbit", email: "hello@orbit.example" },
    subject: "Three launches and a comet you can see this weekend",
    messages: [
      said(
        "Look up after ten on Saturday: the comet sits low in the west, just under the moon.",
        35
      ),
    ],
    unread: true,
  }),
  thread({
    account: 1,
    with: { name: "Tracker", email: "notifications@tracker.example" },
    subject: "3 issues were assigned to you",
    messages: [
      said(
        "Status page copy, Release notes for 4.2, and Broken link in the welcome mail.",
        50
      ),
    ],
    unread: true,
  }),
  thread({
    with: { name: "Mira Patel", email: "mira@hey.example" },
    subject: "Dinner on Saturday?",
    messages: [
      said(
        "We're making the dumplings again. Come around seven if you can, and bring the good bread.",
        95
      ),
    ],
    unread: true,
  }),
  thread({
    account: 1,
    with: { name: "Tom Becker", email: "tom@acme.example" },
    subject: "Q4 budget",
    messages: [
      said(
        "The first draft of the Q4 budget is in the shared folder. Travel is the line I'm least sure about.",
        7 * HOUR
      ),
      wrote(
        "Travel looks right to me. I'd add a little for the offsite in November.",
        3 * HOUR + 20
      ),
    ],
    starred: true,
  }),
  thread({
    with: { name: "Northline Rail", email: "tickets@northline.example" },
    subject: "Your tickets for Friday",
    label: "Travel",
    messages: [
      said(
        "Your tickets are attached. Coach C, seats 41 and 42, leaving at 08:12 from platform 3.",
        5 * HOUR
      ),
    ],
    starred: true,
    attachment: "Tickets.pdf",
  }),
  thread({
    account: 1,
    with: { name: "Calendar", email: "calendar@acme.example" },
    subject: "Invitation: Design review, Wednesday 2pm",
    messages: [
      said(
        "Priya Shah invited you to Design review on Wednesday from 2pm to 3pm in the Blue Room.",
        6 * HOUR
      ),
    ],
    unread: true,
  }),
  thread({
    with: { name: "Jonas Weber", email: "jonas@weber.example" },
    subject: "Photos from the hike",
    messages: [
      said(
        "Finally sorted the photos. The one at the ridge is my favourite, even with the fog.",
        DAY + 4 * HOUR
      ),
      wrote(
        "These are great! Can you send the one by the lake in full size?",
        DAY + 2 * HOUR
      ),
      said("Attached. Same time next month?", DAY),
    ],
    attachment: "Lake.jpg",
  }),
  thread({
    account: 1,
    with: { name: "Ana Costa", email: "ana@clientco.example" },
    subject: "Feedback on the new onboarding",
    label: "Clients",
    messages: [
      said(
        "We tried the new onboarding with five people this week. Four finished it without help, which is a first.",
        DAY + 3 * HOUR
      ),
    ],
  }),
  thread({
    with: { name: "Fernhill Library", email: "desk@fernhill.example" },
    subject: "A book you reserved is ready",
    messages: [
      said(
        "The Overstory is waiting for you at the front desk until next Thursday.",
        DAY + 6 * HOUR
      ),
    ],
  }),
  thread({
    with: { name: "Kettle & Co.", email: "news@kettle.example" },
    subject: "Autumn blends are here",
    messages: [
      said(
        "Smoked pear, roasted chestnut and a very quiet chamomile. Free shipping until Sunday.",
        2 * DAY
      ),
    ],
  }),
  thread({
    account: 1,
    with: { name: "Ben Ito", email: "ben@acme.example" },
    subject: "Lunch?",
    messages: [
      said(
        "The new noodle place opened downstairs. Tomorrow at 12:30?",
        2 * DAY + 2 * HOUR
      ),
    ],
  }),
  thread({
    account: 1,
    with: { name: "Status Weekly", email: "digest@status.example" },
    subject: "This week in infrastructure",
    messages: [
      said(
        "Zero incidents, two faster deploys and one very long post-mortem worth reading.",
        2 * DAY + 5 * HOUR
      ),
    ],
  }),
  thread({
    with: { name: "Mum", email: "mum@example.com" },
    subject: "Call me when you're free",
    messages: [
      said(
        "Nothing urgent, just want to hear about the new job. Sunday afternoon?",
        3 * DAY
      ),
    ],
    starred: true,
  }),
  thread({
    with: { name: "Harbor Bank", email: "statements@harbor.example" },
    subject: "Your statement for September is ready",
    messages: [
      said(
        "Your statement for September is ready to view in the app.",
        4 * DAY
      ),
    ],
  }),
  thread({
    account: 1,
    with: { name: "Payroll", email: "payroll@acme.example" },
    subject: "Your payslip for September",
    messages: [said("Your payslip for September is attached.", 5 * DAY)],
    attachment: "Payslip September.pdf",
  }),
  thread({
    with: { name: "Dr. Lena Ortiz", email: "office@ortiz.example" },
    subject: "Appointment reminder",
    messages: [
      said(
        "A reminder of your appointment on Monday at 9:00. Please arrive ten minutes early.",
        6 * HOUR
      ),
    ],
    box: "snoozed",
  }),
  thread({
    with: { name: "Jonas Weber", email: "jonas@weber.example" },
    subject: "Spare tent",
    messages: [
      wrote(
        "I still have the spare tent if you want it for the trip. It's yours.",
        2 * DAY + 3 * HOUR
      ),
    ],
    box: "archive",
  }),
  thread({
    with: { name: "Pine & Pixel", email: "orders@pinepixel.example" },
    subject: "Your order has shipped",
    messages: [
      said("Your order is on its way and should arrive on Wednesday.", 6 * DAY),
    ],
    box: "archive",
  }),
  thread({
    with: { name: "Casa Azul", email: "stay@casaazul.example" },
    subject: "Your stay in Lisbon",
    label: "Travel",
    messages: [
      said(
        "Check-in is from 3pm. The key is in the box by the blue door; the code is 4821.",
        8 * DAY
      ),
    ],
    box: "archive",
  }),
  thread({
    account: 1,
    with: { name: "Priya Shah", email: "priya@acme.example" },
    subject: "Offsite notes",
    messages: [
      said(
        "Notes from the offsite are in the wiki. The short version: fewer meetings, more demos.",
        9 * DAY
      ),
    ],
    box: "archive",
  }),
  thread({
    with: { name: "Prize Desk", email: "winner@prizes.example" },
    subject: "You may have already won",
    messages: [said("Claim your prize today!", 3 * DAY)],
    box: "trash",
  }),
]

export function isSent(thread: Thread) {
  return thread.box !== "trash" && thread.messages.some((one) => one.mine)
}

export function inMailbox(thread: Thread, mailbox: Mailbox) {
  switch (mailbox) {
    case "inbox":
      return thread.box === "inbox"
    case "unread":
      return thread.box === "inbox" && thread.unread
    case "starred":
      return thread.starred && thread.box !== "trash"
    case "sent":
      return isSent(thread)
    case "archive":
      return thread.box === "archive"
    case "snoozed":
      return thread.box === "snoozed"
    case "trash":
      return thread.box === "trash"
    case "drafts":
    case "spam":
      return false
  }
}

export function latest(thread: Thread) {
  return thread.messages[thread.messages.length - 1]
}

/** "Jonas, me (3)": first names when there are several, in the order they first wrote. */
export function senders(thread: Thread) {
  const names: string[] = []
  for (const message of thread.messages) {
    const name = message.mine ? "me" : thread.with.name
    if (!names.includes(name)) names.push(name)
  }
  const shown =
    names.length > 1
      ? names.map((name) => (name === "me" ? name : name.split(" ")[0]))
      : names
  const count = thread.messages.length > 1 ? ` (${thread.messages.length})` : ""
  return shown.join(", ") + count
}

export function initials(name: string) {
  const words = name.split(" ").filter((word) => /^[A-Za-z]/.test(word))
  return words
    .slice(0, 2)
    .map((word) => word[0].toUpperCase())
    .join("")
}

/** The avatar's colour, picked from the address as the apps do (MailPush's SenderAvatar). */
export function avatarColor(email: string) {
  let sum = 0
  for (const character of email) {
    sum = (sum * 31 + character.codePointAt(0)!) & 0xffff
  }
  return `var(--account-${(sum % 10) + 1})`
}

const time = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
})
const day = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" })
const weekday = new Intl.DateTimeFormat("en-US", { weekday: "short" })

function startOfDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

/** "9:41 AM" today, "Yesterday", "Oct 3" before, as the core's dates.rs. */
export function shortDate(at: Date, now: Date) {
  const days = Math.round(
    (startOfDay(now).getTime() - startOfDay(at).getTime()) / 86_400_000
  )
  if (days === 0) return time.format(at)
  if (days === 1) return "Yesterday"
  return day.format(at)
}

/** "9:41 AM" today, "Oct 3, 9:41 AM" before. */
export function longDate(at: Date, now: Date) {
  const today = startOfDay(now).getTime() === startOfDay(at).getTime()
  return today ? time.format(at) : `${day.format(at)}, ${time.format(at)}`
}

/** When snoozed mail comes back: "Thu 8:00 AM" this week, "Sat, Nov 14, 8:00 AM" after. */
export function snoozeDate(at: Date, now: Date) {
  const soon = at.getTime() - now.getTime() < 6 * 86_400_000
  return soon
    ? `${weekday.format(at)} ${time.format(at)}`
    : `${weekday.format(at)}, ${day.format(at)}, ${time.format(at)}`
}

/** The snooze picker's times, from the page's now. */
export function snoozeTimes(now: Date) {
  const at = (days: number, hour: number) =>
    new Date(now.getFullYear(), now.getMonth(), now.getDate() + days, hour)
  const later = new Date(now)
  later.setHours(now.getHours() + 3, 0, 0, 0)
  const toSaturday = (6 - now.getDay() + 7) % 7 || 7
  const toMonday = (1 - now.getDay() + 7) % 7 || 7
  return [
    { name: "Later today", at: later },
    { name: "This evening", at: at(0, 18) },
    { name: "Tomorrow", at: at(1, 8) },
    { name: "This weekend", at: at(toSaturday, 8) },
    { name: "Next week", at: at(toMonday, 8) },
    {
      name: "Next month",
      at: new Date(now.getFullYear(), now.getMonth() + 1, now.getDate(), 8),
    },
  ].filter(
    (choice) =>
      choice.at > now &&
      (choice.name !== "Later today" || later.getDate() === now.getDate())
  )
}
