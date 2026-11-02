import SwiftUI

/// What both apps' settings read and change, so the Mac's pages and iOS's say and do the same.
extension MailStore {
    func openSettings(_ section: SettingsSection = .general, target: String? = nil) {
        settings = section
        settingsTarget = target
        sidebarOpen = sidebarOpen && keepSidebar
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
        case "connecting": "Connecting"
        default: "Offline. Changes wait and are sent once connected."
        }
    }

    func status(of account: AccountView) -> String {
        switch account.status {
        case "syncing": "Syncing"
        case "reauth": "Sign in again: remove the account and add it back"
        case "error": "Can't reach the provider right now"
        default:
            switch account.provider {
            case "gmail": "Gmail"
            case "outlook": "Outlook"
            default: "JMAP"
            }
        }
    }

    /// The line under an account's name: its status, after its address when the name hides it.
    func subtitle(of account: AccountView) -> String {
        account.name.isEmpty ? status(of: account) : "\(account.address) · \(status(of: account))"
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
                return (id, accounts.count > 1 ? "\(mailbox.name) · \(account.title)" : mailbox.name)
            }
        }
    }

    func summary(of split: CustomSplit) -> String {
        var parts = split.from
        if let label = split.label, let name = splitLabels.first(where: { $0.id == label })?.name { parts.append("label \(name)") }
        return parts.isEmpty ? "Nothing yet" : parts.joined(separator: ", ")
    }

    var customRules: [CustomRule] { CustomRule.all(in: preferences) }

    func newRule() -> RuleDraft {
        let order = (customRules.map(\.order).max() ?? -1) + 1
        let rule = CustomRule(id: UUID().uuidString.lowercased(), from: [], to: [], subject: [], label: nil, skipInbox: false, markRead: false, order: order)
        return RuleDraft(rule, isNew: true)
    }

    func draft(of rule: CustomRule) -> RuleDraft {
        RuleDraft(rule, isNew: false)
    }

    /// Saves the rule, and applies it to the mail already here if asked.
    func saveRule(_ draft: RuleDraft) {
        let rule = draft.written
        setPreference(rule.key, rule.value)
        guard draft.applying else { return }
        Task {
            do {
                report(try await bridge.call("apply_rule", ["rule": rule.value.any], as: ActReply.self))
            } catch {
                show(error.localizedDescription)
            }
        }
    }

    func removeRule(_ rule: CustomRule) {
        setPreference(rule.key, nil)
    }

    /// Puts the rule at `index` among the others.
    func moveRule(_ rule: CustomRule, to index: Int) {
        var reordered = customRules.filter { $0.id != rule.id }
        reordered.insert(rule, at: min(index, reordered.count))
        for (order, rule) in reordered.enumerated() where rule.order != order {
            var moved = rule
            moved.order = order
            setPreference(moved.key, moved.value)
        }
    }

    /// "From @github.com, to dux-shell@noreply.github.com → dux-shell, skip inbox".
    func summary(of rule: CustomRule) -> String {
        var conditions: [String] = []
        if !rule.from.isEmpty { conditions.append("from \(rule.from.joined(separator: " or "))") }
        if !rule.to.isEmpty { conditions.append("to \(rule.to.joined(separator: " or "))") }
        if !rule.subject.isEmpty { conditions.append("subject has \(rule.subject.joined(separator: " or "))") }
        var actions: [String] = []
        if let label = rule.label, let name = splitLabels.first(where: { $0.id == label })?.name { actions.append(name) }
        if rule.skipInbox { actions.append("skip inbox") }
        if rule.markRead { actions.append("mark read") }
        let joined = conditions.joined(separator: ", ")
        let looks = joined.isEmpty ? "Nothing yet" : joined.prefix(1).uppercased() + String(joined.dropFirst())
        guard !actions.isEmpty else { return looks }
        return "\(looks) → \(actions.joined(separator: ", "))"
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

    var swipeCommands: [ThreadCommand] { SwipePreference.commands(in: preferences) }

    /// Keeps one to three swipe commands, removed while they are the usual ones.
    func setSwipeCommands(_ commands: [ThreadCommand]) {
        guard (1...SwipePreference.most).contains(commands.count) else { return }
        setPreference(SwipePreference.key, commands == SwipePreference.standard ? nil : SwipePreference.value(commands))
    }

    func moveSwipeCommand(_ command: ThreadCommand, to index: Int) {
        var commands = swipeCommands.filter { $0 != command }
        commands.insert(command, at: min(index, commands.count))
        setSwipeCommands(commands)
    }

    /// The commands a swipe could have and doesn't yet.
    var swipeChoicesLeft: [ThreadCommand] {
        SwipePreference.choices.filter { !swipeCommands.contains($0) }
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

    /// Who keeps the account's mail, as a sentence names them.
    func providerName(_ account: AccountView) -> String {
        switch account.provider {
        case "gmail": "Gmail"
        case "outlook": "Outlook"
        default: "your provider"
        }
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

/// A rule as its editor holds it, its conditions as typed.
struct RuleDraft: Identifiable {
    var rule: CustomRule
    let isNew: Bool
    var from: String
    var to: String
    var subject: String
    /// Also does what it does to the mail already here, once saved.
    var applying: Bool

    var id: String { rule.id }

    init(_ rule: CustomRule, isNew: Bool) {
        self.rule = rule
        self.isNew = isNew
        from = rule.from.joined(separator: ", ")
        to = rule.to.joined(separator: ", ")
        subject = rule.subject.joined(separator: ", ")
        applying = isNew
    }

    var written: CustomRule {
        var rule = rule
        rule.from = CustomSplit.senders(from)
        rule.to = CustomSplit.senders(to)
        rule.subject = CustomRule.words(subject)
        return rule
    }

    /// A rule is kept once it matches something and does something.
    var canSave: Bool {
        let rule = written
        return rule.hasCondition && (rule.label != nil || rule.skipInbox || rule.markRead)
    }
}
