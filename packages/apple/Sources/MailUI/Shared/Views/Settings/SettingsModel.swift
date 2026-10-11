import SwiftUI

/// What both apps' settings read and change, so the Mac's pages and iOS's say and do the same.
extension MailStore {
    func openSettings(_ section: SettingsSection = .general, target: String? = nil) {
        settings = section
        settingsTarget = target
        sidebarOpen = false
        Task { await loadPreferences() }
    }

    func closeSettings() {
        settings = nil
        settingsTarget = nil
        settingsQuery = ""
    }

    /// A preference that is on or off, removed while it is as it starts.
    func flag(_ key: String, default on: Bool) -> Binding<Bool> {
        Binding(
            get: { self.preferences[key]?.bool ?? on },
            set: { self.setPreference(key, $0 == on ? nil : .bool($0)) }
        )
    }

    func notifies(_ account: AccountView) -> Binding<Bool> {
        flag("notify:\(account.id)", default: true)
    }

    var connectionText: String {
        switch connection {
        case "online": "Connected"
        case "connecting": "Connecting…"
        default: "Offline. Changes wait and are sent once connected."
        }
    }

    func status(of account: AccountView) -> String {
        switch account.status {
        case "syncing": "Syncing…"
        case "reauth": "Sign in again: remove the account and add it back"
        case "error": "Can't reach the provider right now"
        default: account.provider == "gmail" ? "Gmail" : "JMAP"
        }
    }

    func confirmRemoving(_ account: AccountView) {
        confirmation = Confirmation(
            title: "Remove \(account.address)?", message: "Its mail leaves this \(Platform.device) and your other devices. You can add it again.",
            action: "Remove"
        ) { [weak self] in self?.removeAccount(account.id) }
    }

    func confirmSigningOut() {
        confirmation = Confirmation(
            title: "Sign out of Wonnet?", message: "Every account leaves this \(Platform.device). Signing in again brings them back.", action: "Sign out"
        ) { [weak self] in
            self?.closeSettings()
            self?.signOut()
        }
    }

    /// The senders and domains whose new mail goes to the trash.
    var blockedSenders: [String] {
        preferences.filter { $0.key.hasPrefix("blocked:") && $0.value.bool != false }
            .map { String($0.key.dropFirst("blocked:".count)) }
            .sorted()
    }

    func unblock(_ address: String) {
        setPreference("blocked:\(address)", nil)
    }

    var splitInboxOn: Binding<Bool> { flag("split_inbox", default: false) }

    var customSplits: [CustomSplit] { CustomSplit.all(in: preferences) }

    func newSplit() -> CustomSplit {
        let order = (customSplits.map(\.order).max() ?? -1) + 1
        return CustomSplit(id: UUID().uuidString.lowercased(), name: "", from: [], label: nil, order: order)
    }

    func saveSplit(_ split: CustomSplit) {
        setPreference(split.key, split.value)
    }

    func removeSplit(_ split: CustomSplit) {
        setPreference(split.key, nil)
    }

    /// Puts the split at `index` among the others.
    func moveSplit(_ split: CustomSplit, to index: Int) {
        var reordered = customSplits.filter { $0.id != split.id }
        reordered.insert(split, at: min(index, reordered.count))
        for (order, split) in reordered.enumerated() where split.order != order {
            var moved = split
            moved.order = order
            setPreference(moved.key, moved.value)
        }
    }

    /// Every account's labels, by id, for a split to take mail of.
    var splitLabels: [(id: String, name: String)] {
        accounts.flatMap { account in
            account.mailboxes.compactMap { mailbox -> (id: String, name: String)? in
                guard let id = Self.labelID(mailbox.id) else { return nil }
                return (id, accounts.count > 1 ? "\(mailbox.name) · \(account.address)" : mailbox.name)
            }
        }
    }

    func summary(of split: CustomSplit) -> String {
        var parts = split.from
        if let label = split.label, let name = splitLabels.first(where: { $0.id == label })?.name { parts.append("label \(name)") }
        return parts.isEmpty ? "Nothing yet" : parts.joined(separator: ", ")
    }

    /// Puts the tab at `index` among the others.
    func moveTab(_ tab: Mailbox, to index: Int) {
        var ids = savedTabs.map(\.id).filter { $0 != tab.id }
        ids.insert(tab.id, at: min(index, ids.count))
        setTabs(ids)
    }

    /// The mailboxes that could be tabs and aren't yet: those of every account together, then each account's.
    var mailboxesForTabs: (unified: [Mailbox], accounts: [(account: AccountView, mailboxes: [Mailbox])]) {
        let unified = unified.filter { !isTab($0.id) }
        let accounts = accounts.compactMap { account -> (account: AccountView, mailboxes: [Mailbox])? in
            let left = account.mailboxes.filter { !isTab($0.id) }
            return left.isEmpty ? nil : (account, left)
        }
        return (unified, accounts)
    }

    var snippets: [Snippet] { Snippet.all(in: preferences) }

    func saveSnippet(_ snippet: Snippet) {
        var snippet = snippet
        snippet.name = snippet.name.trimmingCharacters(in: .whitespaces)
        setPreference(snippet.key, snippet.value)
    }

    /// The signature the provider keeps for the account, used while the user's own is empty.
    func providedSignature(_ account: AccountView) -> String? {
        account.identities.first?.signature.flatMap { $0.isEmpty ? nil : $0 }
    }

    func ownSignature(_ account: AccountView) -> String {
        preferences["signature:\(account.id)"]?.string ?? ""
    }

    func setSignature(_ text: String, for account: AccountView) {
        setPreference("signature:\(account.id)", text.isEmpty ? nil : .string(text))
    }

    static let undoDelays = [0, 5, 10, 20, 30]

    static func undoDelayName(_ seconds: Int) -> String {
        seconds == 0 ? "Off" : "\(seconds) s"
    }
}

extension CustomSplit {
    /// The senders as they are typed: addresses and `@domains`, split on commas, spaces or lines.
    static func senders(_ text: String) -> [String] {
        text.split(whereSeparator: { ", ;\n".contains($0) }).map { $0.lowercased() }
    }
}
