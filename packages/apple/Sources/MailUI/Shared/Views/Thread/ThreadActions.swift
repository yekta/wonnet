import SwiftUI

/// What can be done to threads, alike from the keys, the toolbars, the menus and the palette.
enum ThreadCommand: String, CaseIterable, Identifiable {
    case archive, inbox, trash, deleteForever, spam, star, read, markRead, snooze, label, move, mute, unsubscribe, block, print, reply, replyAll, forward, expandAll

    var id: String { rawValue }

    /// Its name for these threads: "Unstar" when they all are starred.
    @MainActor func title(_ store: MailStore, _ threads: [String]) -> String {
        switch self {
        case .archive: "Archive"
        case .inbox: "Move to Inbox"
        case .trash: "Trash"
        case .deleteForever: "Delete forever"
        case .spam: "Spam"
        case .star: store.allStarred(threads) ? "Unstar" : "Star"
        case .read: store.anyUnread(threads) ? "Mark read" : "Mark unread"
        case .markRead: "Mark read"
        case .snooze: store.remindsInsteadOfSnoozing ? "Remind me" : "Snooze"
        case .label: "Label"
        case .move: "Move to"
        case .mute: store.isMuted(threads) ? "Unmute" : "Mute"
        case .unsubscribe: "Unsubscribe"
        case .block: "Block sender"
        case .print: "Print"
        case .reply: "Reply"
        case .replyAll: "Reply all"
        case .forward: "Forward"
        case .expandAll: store.anyMessageFolded ? "Expand all messages" : "Collapse all messages"
        }
    }

    var symbol: Symbol {
        switch self {
        case .archive: .archive
        case .inbox: .archiveRestore
        case .trash, .deleteForever: .trash
        case .spam: .shieldAlert
        case .star: .star
        case .read: .mailOpen
        case .markRead: .checkCheck
        case .snooze: .clock
        case .label: .tag
        case .move: .folderInput
        case .mute: .bellOff
        case .unsubscribe: .mailMinus
        case .block: .ban
        case .print: .printer
        case .reply: .reply
        case .replyAll: .replyAll
        case .forward: .forward
        case .expandAll: .chevronsUpDown
        }
    }

    /// What doesn't fit on a toolbar: in its "more" menu.
    static let more: [ThreadCommand] = [.star, .inbox, .move, .mute, .unsubscribe, .spam, .block, .deleteForever, .print, .expandAll]

    /// What a bar over selected threads offers, and what its "more" menu does.
    static let selection: [ThreadCommand] = [.archive, .trash, .snooze, .label, .read]
    static let selectionMore = more.filter { ![.print, .expandAll].contains($0) }

    /// What only the open thread can have done to it, one at a time.
    static let openOnly: [ThreadCommand] = [.reply, .replyAll, .forward, .print, .expandAll]

    /// Its name where it is picked for a swipe, which does it to any thread.
    var swipeName: String {
        switch self {
        case .read: "Read or unread"
        case .star: "Star or unstar"
        case .snooze: "Snooze"
        case .move: "Move to"
        case .mute: "Mute"
        default: rawValue.capitalized
        }
    }

    /// Its colour on a swipe, and its icon's on that colour.
    var swipeTint: (fill: ThemeColor, glyph: ThemeColor) {
        switch self {
        case .read: (Tokens.primary, Tokens.primaryForeground)
        case .snooze: (Tokens.warning, Tokens.warningForeground)
        case .archive: (Tokens.success, Tokens.successForeground)
        case .trash, .spam: (Tokens.destructive, Tokens.destructiveForeground)
        case .star: (Tokens.star, Tokens.starForeground)
        case .mute: (Tokens.pending, Tokens.pendingForeground)
        default: (Tokens.process, Tokens.processForeground)
        }
    }

    /// Its icon on a swipe over this row: what it would make of it.
    func swipeSymbol(_ row: ThreadRow) -> Symbol {
        switch self {
        case .read: row.unread ? .mailOpen : .mail
        default: symbol
        }
    }

    /// Whether doing it takes the thread out of the list at once, so its row slides away.
    var takesRowAway: Bool {
        [.archive, .trash, .spam, .mute].contains(self)
    }
}

/// What can be done to the open thread: archive, trash, snooze, label, mark unread, and more.
struct ThreadActions: View {
    @Environment(MailStore.self) private var store
    let thread: String

    var body: some View {
        HStack(spacing: 0) {
            ForEach([ThreadCommand.archive, .trash, .snooze, .label, .read]) { command in
                IconButton(symbol: command.symbol, help: help(command)) { store.run(command, on: [thread]) }
            }
            IconMenu(symbol: .ellipsis, help: "More") {
                ThreadCommandButtons(commands: ThreadCommand.more, threads: [thread])
            }
        }
    }

    private func help(_ command: ThreadCommand) -> String {
        store.keyHelp(command.title(store, [thread]), command.rawValue)
    }
}

/// Commands as the buttons of a menu.
struct ThreadCommandButtons: View {
    @Environment(MailStore.self) private var store
    let commands: [ThreadCommand]
    let threads: [String]

    var body: some View {
        ForEach(commands) { command in
            Button {
                store.run(command, on: threads)
            } label: {
                Label { Text(command.title(store, threads)) } icon: { Image(command.symbol, size: 15) }
            }
        }
    }
}
