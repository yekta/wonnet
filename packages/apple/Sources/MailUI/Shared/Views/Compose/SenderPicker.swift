import SwiftUI

/// An address a message can be from: one of an account's identities, the first its own.
struct Sender: Hashable, Identifiable {
    let account: AccountView
    let identity: Identity
    let first: Bool

    var id: String { account.id + "\n" + identity.email }
    var name: String { identity.name ?? identity.email }
    var color: Color { Theme.accountColor(account.color).color }

    /// Every address the accounts can send as, in the sidebar's order.
    static func all(in accounts: [AccountView]) -> [Sender] {
        accounts.flatMap { account in
            let identities = account.identities.isEmpty ? [Identity(name: nil, email: account.address, signature: nil)] : account.identities
            return identities.enumerated().map { index, identity in Sender(account: account, identity: identity, first: index == 0) }
        }
    }

    /// Whether every word typed is in its name, its address or its account's name.
    func matches(_ query: String) -> Bool {
        let haystack = [identity.name ?? "", identity.email, account.title].joined(separator: " ")
        return query.split(whereSeparator: \.isWhitespace).allSatisfy { haystack.localizedCaseInsensitiveContains($0) }
    }
}

/// Which address the message goes from, found by typing a few letters of it, or of its account.
struct SenderPicker: View {
    @Environment(\.dismiss) private var dismiss
    let senders: [Sender]
    let current: Sender?
    let pick: (Sender) -> Void
    @State private var query = ""
    @State private var highlighted = 0

    private var found: [Sender] {
        let typed = query.trimmingCharacters(in: .whitespaces)
        guard !typed.isEmpty else { return senders }
        return senders.filter { $0.matches(typed) }
    }

    var body: some View {
        let found = found
        Sheet(title: "Send as") {
            SearchedList(
                field: SearchField(
                    placeholder: "Find an address", text: $query, symbol: .atSign,
                    submit: { choose(found, highlighted) }, move: { highlighted = highlighted.moved(by: $0, in: found.count) },
                    escape: { dismiss() }
                )
            ) {
                ChoiceList(items: found, highlighted: highlighted, empty: "No address by that name.") { index, sender in
                    ChoiceRow(
                        title: sender.name, detail: sender.identity.name == nil ? sender.account.title : sender.identity.email, dot: sender.color,
                        checked: sender == current, highlighted: index == highlighted
                    ) {
                        choose(found, index)
                    }
                }
            }
        }
        .onChange(of: query) { highlighted = 0 }
        .onAppear {
            guard let current, let index = senders.firstIndex(of: current) else { return }
            highlighted = index
        }
    }

    private func choose(_ found: [Sender], _ index: Int) {
        guard found.indices.contains(index) else { return }
        pick(found[index])
        dismiss()
    }
}
