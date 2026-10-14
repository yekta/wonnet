import Foundation

/// A page of the settings, listed beside it on the Mac and pushed from the list on iOS.
enum SettingsSection: String, CaseIterable, Identifiable, Hashable {
    case general, accounts, inbox, reading, writing, keybindings

    var id: String { rawValue }

    /// The sections this app has: the keys' only where there is a keyboard to set.
    static var shown: [SettingsSection] {
        #if os(macOS)
        allCases
        #else
        allCases.filter { $0 != .keybindings }
        #endif
    }

    var title: String {
        switch self {
        case .general: "General"
        case .accounts: "Accounts"
        case .inbox: "Inbox"
        case .reading: "Reading"
        case .writing: "Writing"
        case .keybindings: "Keybindings"
        }
    }

    var symbol: Symbol {
        switch self {
        case .general: .settings
        case .accounts: .atSign
        case .inbox: .inbox
        case .reading: .mailOpen
        case .writing: .pencil
        case .keybindings: .keyboard
        }
    }
}

/// One group of settings, as the settings' search finds it. Its `id` is the group's on its page.
struct SettingsEntry: Identifiable, Hashable {
    let id: String
    let title: String
    let section: SettingsSection
    /// Words it is also found by, beyond its title and its section's.
    let keywords: String

    static let all: [SettingsEntry] = [
        SettingsEntry(id: "appearance", title: "Theme", section: .general, keywords: "appearance light dark system mode"),
        SettingsEntry(id: "updates", title: "Updates", section: .general, keywords: "version check for updates release about"),
        SettingsEntry(id: "server", title: "Server", section: .general, keywords: "connection connected offline sync"),
        SettingsEntry(id: "accounts", title: "Accounts", section: .accounts, keywords: "add remove gmail jmap fastmail colour color notifications"),
        SettingsEntry(id: "signatures", title: "Signatures", section: .accounts, keywords: "sign off footer"),
        SettingsEntry(id: "sign-out", title: "Sign out", section: .accounts, keywords: "log out leave"),
        SettingsEntry(id: "split-inbox", title: "Split Inbox", section: .inbox, keywords: "important other newsletters notifications splits"),
        SettingsEntry(id: "tabs", title: "Tabs", section: .inbox, keywords: "mailboxes over the list order"),
        SettingsEntry(id: "mail", title: "Dark mode for mail", section: .reading, keywords: "designed newsletters colours remote images pictures privacy tracking"),
        SettingsEntry(id: "blocked", title: "Blocked senders", section: .reading, keywords: "block unblock trash spam"),
        SettingsEntry(id: "sending", title: "Undo send", section: .writing, keywords: "delay cancel sending"),
        SettingsEntry(id: "snippets", title: "Snippets", section: .writing, keywords: "templates canned replies text"),
        SettingsEntry(id: "keybindings", title: "Keybindings", section: .keybindings, keywords: "keyboard shortcuts keys hotkeys superhuman"),
    ]

    /// The groups only a touch screen has.
    static var touch: [SettingsEntry] {
        #if os(iOS)
        [SettingsEntry(id: "swipe", title: "Swipe Left", section: .inbox, keywords: "swipe actions gestures archive snooze read unread")]
        #else
        []
        #endif
    }

    /// The groups this app has.
    static var shown: [SettingsEntry] {
        (all + touch).filter { SettingsSection.shown.contains($0.section) }
    }

    /// The groups every word of the query is found in, and the commands of the keys it names.
    static func matching(_ query: String) -> [SettingsEntry] {
        let words = query.split(whereSeparator: \.isWhitespace).map(String.init)
        guard !words.isEmpty else { return [] }
        let keys = SettingsSection.shown.contains(.keybindings)
            ? KeyCommand.all.map { SettingsEntry(id: "key-\($0.id)", title: $0.title, section: .keybindings, keywords: "shortcut keys \($0.group)") }
            : []
        return (shown + keys).filter { entry in
            let text = "\(entry.title) \(entry.section.title) \(entry.keywords)"
            return words.allSatisfy { text.localizedCaseInsensitiveContains($0) }
        }
    }
}
