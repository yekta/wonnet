import SwiftUI

/// What can be done to threads, alike from the keys, the toolbars, the menus and the palette.
enum ThreadCommand: String, CaseIterable, Identifiable {
    case archive, inbox, trash, deleteForever, spam, star, read, markRead, snooze, label, move, mute, unsubscribe, block, print, reply, replyAll, forward

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
        }
    }

    /// What doesn't fit on a toolbar: in its "more" menu.
    static let more: [ThreadCommand] = [.star, .inbox, .move, .mute, .unsubscribe, .spam, .block, .deleteForever, .print]
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
