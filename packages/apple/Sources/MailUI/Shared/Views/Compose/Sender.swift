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

    /// The sender as the From field's combobox offers it: its name, then its address, or its
    /// account's name when it has none, found by any of them.
    var choice: Choice {
        Choice(
            id: id, title: name, detail: identity.name == nil ? account.title : identity.email, dot: color,
            keywords: [identity.email, account.title].joined(separator: " ")
        )
    }
}
