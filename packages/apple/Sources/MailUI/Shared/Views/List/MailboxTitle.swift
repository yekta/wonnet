import SwiftUI

/// The open mailbox's name over the list: "Inbox · Work", the dot fainter than the names.
struct MailboxTitle: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        text.textStyle(.subheading).lineLimit(1)
    }

    private var text: Text {
        let title = store.baseMailboxTitle
        guard let account = title.account else { return Text(title.name) }
        return Text(title.name)
            + Text(" · ").foregroundStyle(Tokens.mutedForeground.color)
            + Text(account)
    }
}
