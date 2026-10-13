import SwiftUI

/// The open mailbox's name over the list: "Inbox · Work", the dot fainter than the names, with
/// a spinner after it while its mail is still coming. The spinner's slot is always there, so the
/// name doesn't move; on iOS, where the name is centred, a blank slot before it keeps it centred.
struct MailboxTitle: View {
    @Environment(MailStore.self) private var store

    private static let slot: CGFloat = 14 * Platform.scale

    var body: some View {
        HStack(spacing: Space.xs) {
            #if os(iOS)
            Color.clear.frame(width: Self.slot, height: Self.slot)
            #endif
            text.textStyle(.subheading).lineLimit(1)
            if store.mailboxSyncing {
                Spinner(size: Self.slot)
            } else {
                Color.clear.frame(width: Self.slot, height: Self.slot)
            }
        }
    }

    private var text: Text {
        let title = store.baseMailboxTitle
        guard let account = title.account else { return Text(title.name) }
        return Text(title.name)
            + Text(" · ").foregroundStyle(Tokens.mutedForeground.color)
            + Text(account)
    }
}
