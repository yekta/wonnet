import SwiftUI

/// Over the list: the split inbox's tabs, the filter the list is narrowed to, and emptying Trash
/// or Spam.
struct ListHeader: View {
    @Environment(MailStore.self) private var store

    static let height = 38 * Platform.scale

    static func shows(_ store: MailStore) -> Bool {
        store.searchRows == nil && (!store.splits.isEmpty || store.filter != nil || store.canEmpty)
    }

    var body: some View {
        if Self.shows(store) {
            HStack(spacing: Space.m) {
                ScrollView(.horizontal, showsIndicators: false) {
                    TabStrip(
                        tabs: store.splits, selected: chosenSplit, title: \.name, count: \.unread,
                        pick: { store.select(mailbox: $0.mailbox) }
                    )
                    .padding(.horizontal, Space.l)
                }
                if let filter = store.filter {
                    Chip(title: filter == .unread ? "Unread" : "Starred", symbol: .listFilter, help: "Show everything", remove: { store.setFilter(nil) })
                        .padding(.trailing, Space.l)
                }
                if store.canEmpty {
                    ActionButton(title: "Empty \(store.mailboxName)", variant: .ghost, action: store.emptyMailbox)
                        .padding(.trailing, Space.l)
                }
            }
            .frame(height: Self.height)
            .background(Tokens.background.color)
            .rule(.bottom)
        }
    }

    /// The tab on screen: the one asked for, else the first, which the core shows for the inbox.
    private var chosenSplit: String? {
        store.splits.first(where: { $0.mailbox == store.mailbox })?.mailbox ?? store.splits.first?.mailbox
    }
}

/// An empty mailbox: a calm line instead of a list.
struct EmptyList: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        EmptyState(symbol: store.searchRows != nil ? .search : .inbox, message: message)
    }

    private var message: String {
        if store.searchRows != nil { return "Nothing matches." }
        if store.filter == .unread || store.inUnread { return "Nothing unread." }
        if store.filter == .starred { return "Nothing starred." }
        if store.mailboxSyncing { return "Fetching your mail…" }
        return store.baseMailbox.hasSuffix("inbox") ? "All done. Enjoy the quiet." : "Nothing here."
    }
}

/// How many threads are selected, after the circle that selects all of them or none.
struct SelectionCount: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let all = store.selection.count == store.visibleRows.count
        HStack(spacing: 0) {
            SelectCircle(checked: all, help: all ? "Select none" : store.keyHelp("Select all", "selectAll")) {
                all ? store.clearSelection() : store.selectAll()
            }
            Text("\(store.selection.count) selected").textStyle(.labelStrong).lineLimit(1).fixedSize()
        }
    }
}

/// What can be done to all the selected threads, named while there is room, and the way out.
struct SelectionActions: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let threads = store.targets
        HStack(spacing: 0) {
            ViewThatFits(in: .horizontal) {
                commands(threads, named: true)
                commands(threads, named: false)
            }
            IconButton(symbol: .x, help: "Clear the selection (Esc)", circled: false, action: store.clearSelection)
        }
    }

    private func commands(_ threads: [String], named: Bool) -> some View {
        HStack(spacing: 0) {
            ForEach(store.applicable(ThreadCommand.selection, to: threads)) { command in
                let title = command.title(store, threads)
                let help = store.keyHelp(title, command.rawValue)
                if named {
                    ActionButton(title: title, symbol: command.symbol, variant: .ghost) { store.run(command, on: threads) }.help(help)
                } else {
                    IconButton(symbol: command.symbol, help: help, circled: false) { store.run(command, on: threads) }
                }
            }
            let more = store.applicable(ThreadCommand.selectionMore, to: threads)
            if !more.isEmpty {
                IconMenu(symbol: .ellipsis, help: "More", circled: false) {
                    ThreadCommandButtons(commands: more, threads: threads)
                }
            }
        }
    }
}
