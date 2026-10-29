import SwiftUI

/// Where a command's keys work: anywhere, over the list, or in an open thread. The same keys can
/// do one thing over the list and another in a thread, as Enter opens a row and replies to all.
enum KeyScope {
    case everywhere, list, thread

    func overlaps(_ other: KeyScope) -> Bool {
        self == .everywhere || other == .everywhere || self == other
    }
}

/// Something the keys can do, with the keys it has until the user changes them. The defaults are
/// Superhuman's. A key is written as the settings store it: `e`, `shift+e`, `mod+k` (⌘K),
/// `ctrl+1`, `g i` (G, then I). A sign is the key it is on, on a US keyboard: `shift+3` is #.
struct KeyCommand: Identifiable, Hashable {
    let id: String
    let title: String
    let group: String
    var scope: KeyScope = .everywhere
    let defaults: [String]

    var preferenceKey: String { "keys:\(id)" }

    static let groups = ["Moving around", "Selecting", "Conversations", "Writing", "Going to", "Wonnet"]

    static let all: [KeyCommand] = [
        KeyCommand(id: "next", title: "Next conversation", group: "Moving around", defaults: ["j", "arrowdown"]),
        KeyCommand(id: "previous", title: "Previous conversation", group: "Moving around", defaults: ["k", "arrowup"]),
        KeyCommand(id: "open", title: "Open", group: "Moving around", scope: .list, defaults: ["enter", "o"]),
        KeyCommand(id: "top", title: "Jump to the top", group: "Moving around", scope: .list, defaults: ["mod+arrowup"]),
        KeyCommand(id: "bottom", title: "Jump to the bottom", group: "Moving around", scope: .list, defaults: ["mod+arrowdown"]),
        KeyCommand(id: "nextMessage", title: "Next message", group: "Moving around", scope: .thread, defaults: ["n"]),
        KeyCommand(id: "previousMessage", title: "Previous message", group: "Moving around", scope: .thread, defaults: ["p"]),
        KeyCommand(id: "expandAll", title: "Expand or collapse all messages", group: "Moving around", scope: .thread, defaults: ["shift+o"]),
        KeyCommand(id: "nextSplit", title: "Next split", group: "Moving around", scope: .list, defaults: ["tab"]),
        KeyCommand(id: "previousSplit", title: "Previous split", group: "Moving around", scope: .list, defaults: ["shift+tab"]),
        KeyCommand(id: "search", title: "Search", group: "Moving around", defaults: ["/"]),
        KeyCommand(id: "palette", title: "Command palette", group: "Moving around", defaults: ["mod+k"]),

        KeyCommand(id: "select", title: "Select", group: "Selecting", scope: .list, defaults: ["x"]),
        KeyCommand(id: "selectNext", title: "Select the next too", group: "Selecting", scope: .list, defaults: ["shift+j", "shift+arrowdown"]),
        KeyCommand(id: "selectPrevious", title: "Select the previous too", group: "Selecting", scope: .list, defaults: ["shift+k", "shift+arrowup"]),
        KeyCommand(id: "selectAll", title: "Select all", group: "Selecting", scope: .list, defaults: ["mod+a"]),

        KeyCommand(id: "archive", title: "Archive", group: "Conversations", defaults: ["e"]),
        KeyCommand(id: "inbox", title: "Move to Inbox", group: "Conversations", defaults: ["shift+e"]),
        KeyCommand(id: "snooze", title: "Snooze or remind me", group: "Conversations", defaults: ["h"]),
        KeyCommand(id: "star", title: "Star", group: "Conversations", defaults: ["s"]),
        KeyCommand(id: "read", title: "Mark read or unread", group: "Conversations", defaults: ["u"]),
        KeyCommand(id: "markRead", title: "Mark read", group: "Conversations", defaults: ["shift+i"]),
        KeyCommand(id: "trash", title: "Trash", group: "Conversations", defaults: ["shift+3", "backspace"]),
        KeyCommand(id: "spam", title: "Spam", group: "Conversations", defaults: ["shift+1"]),
        KeyCommand(id: "mute", title: "Mute", group: "Conversations", defaults: ["shift+m"]),
        KeyCommand(id: "label", title: "Label", group: "Conversations", defaults: ["l"]),
        KeyCommand(id: "removeLabel", title: "Remove this label", group: "Conversations", defaults: ["y"]),
        KeyCommand(id: "move", title: "Move to", group: "Conversations", defaults: ["v"]),
        KeyCommand(id: "unsubscribe", title: "Unsubscribe", group: "Conversations", defaults: ["mod+u"]),
        KeyCommand(id: "undo", title: "Undo", group: "Conversations", defaults: ["z", "mod+z"]),
        KeyCommand(id: "print", title: "Print", group: "Conversations", defaults: ["mod+p"]),

        KeyCommand(id: "compose", title: "New message", group: "Writing", defaults: ["c", "mod+n"]),
        KeyCommand(id: "reply", title: "Reply", group: "Writing", defaults: ["r"]),
        KeyCommand(id: "replyAll", title: "Reply all", group: "Writing", defaults: ["a"]),
        KeyCommand(id: "replyAllInThread", title: "Reply all in an open thread", group: "Writing", scope: .thread, defaults: ["enter"]),
        KeyCommand(id: "forward", title: "Forward", group: "Writing", defaults: ["f"]),

        KeyCommand(id: "go.inbox", title: "Inbox", group: "Going to", defaults: ["g i"]),
        KeyCommand(id: "go.starred", title: "Starred", group: "Going to", defaults: ["g s"]),
        KeyCommand(id: "go.sent", title: "Sent", group: "Going to", defaults: ["g t"]),
        KeyCommand(id: "go.drafts", title: "Drafts", group: "Going to", defaults: ["g d"]),
        KeyCommand(id: "go.archive", title: "Archive", group: "Going to", defaults: ["g e"]),
        KeyCommand(id: "go.snoozed", title: "Snoozed", group: "Going to", defaults: ["g h"]),
        KeyCommand(id: "go.spam", title: "Spam", group: "Going to", defaults: ["g shift+1"]),
        KeyCommand(id: "go.trash", title: "Trash", group: "Going to", defaults: ["g shift+3"]),
        KeyCommand(id: "go.label", title: "A label", group: "Going to", defaults: ["g l"]),
        KeyCommand(id: "unreadOnly", title: "Unread only", group: "Going to", scope: .list, defaults: ["shift+u"]),
        KeyCommand(id: "starredOnly", title: "Starred only", group: "Going to", scope: .list, defaults: ["shift+s"]),
        KeyCommand(id: "go.account.0", title: "All inboxes", group: "Going to", defaults: ["ctrl+0"]),
    ] + (1...9).map { number in
        KeyCommand(id: "go.account.\(number)", title: "Account \(number)'s inbox", group: "Going to", defaults: ["ctrl+\(number)"])
    } + [
        KeyCommand(id: "zero", title: "Get Me To Zero", group: "Wonnet", scope: .list, defaults: []),
        KeyCommand(id: "settings", title: "Settings", group: "Wonnet", defaults: ["mod+,"]),
        KeyCommand(id: "keybindings", title: "Keybindings", group: "Wonnet", defaults: ["shift+/", "mod+/"]),
    ]

    static func named(_ id: String) -> KeyCommand? {
        all.first { $0.id == id }
    }
}

/// Writing and reading keys as `KeyCommand` stores them.
enum Keys {
    private static let named: [String: String] = [
        "enter": "↩", "tab": "⇥", "space": "Space", "backspace": "⌫", "delete": "⌦", "escape": "Esc",
        "arrowup": "↑", "arrowdown": "↓", "arrowleft": "←", "arrowright": "→",
        "home": "↖", "end": "↘", "pageup": "⇞", "pagedown": "⇟",
    ]
    /// What Shift types on each sign's key, on a US keyboard: `shift+3` reads #.
    private static let shifted: [String: String] = [
        "1": "!", "2": "@", "3": "#", "4": "$", "5": "%", "6": "^", "7": "&", "8": "*", "9": "(", "0": ")",
        "-": "_", "=": "+", "[": "{", "]": "}", "\\": "|", ";": ":", "'": "\"", ",": "<", ".": ">", "/": "?", "`": "~",
    ]

    /// The strokes of a key, one cap each: `g shift+3` reads G, #.
    static func caps(_ key: String) -> [String] {
        key.split(separator: " ").map { cap(String($0)) }
    }

    /// The key as a line of text says it: "⌘K", "G I".
    static func label(_ key: String) -> String {
        caps(key).joined(separator: " ")
    }

    private static func cap(_ stroke: String) -> String {
        var parts = stroke.split(separator: "+").map(String.init)
        guard let name = parts.popLast() else { return stroke }
        let held = Set(parts)
        if held == ["shift"], let sign = shifted[name] { return sign }
        let symbols = [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("mod", "⌘")].filter { held.contains($0.0) }.map(\.1)
        return symbols.joined() + (named[name] ?? name.uppercased())
    }

    /// A single stroke with ⌘ or ⌃, which a menu item can show and run while typing.
    static func menuShortcut(_ key: String) -> KeyboardShortcut? {
        guard !key.contains(" ") else { return nil }
        let parts = key.split(separator: "+").map(String.init)
        guard let name = parts.last, parts.contains("mod") || parts.contains("ctrl") else { return nil }
        let equivalent: KeyEquivalent
        switch name {
        case "enter": equivalent = .return
        case "tab": equivalent = .tab
        case "space": equivalent = .space
        case "escape": equivalent = .escape
        case "backspace": equivalent = .delete
        case "delete": equivalent = .deleteForward
        case "arrowup": equivalent = .upArrow
        case "arrowdown": equivalent = .downArrow
        case "arrowleft": equivalent = .leftArrow
        case "arrowright": equivalent = .rightArrow
        default:
            guard name.count == 1, let character = name.first else { return nil }
            equivalent = KeyEquivalent(character)
        }
        var modifiers: EventModifiers = []
        if parts.contains("mod") { modifiers.insert(.command) }
        if parts.contains("ctrl") { modifiers.insert(.control) }
        if parts.contains("alt") { modifiers.insert(.option) }
        if parts.contains("shift") { modifiers.insert(.shift) }
        return KeyboardShortcut(equivalent, modifiers: modifiers)
    }

    /// Whether two keys can't both be kept: the same, or one the start of the other (`g` and `g i`).
    static func clash(_ one: String, _ other: String) -> Bool {
        one == other || other.hasPrefix(one + " ") || one.hasPrefix(other + " ")
    }
}

extension MailStore {
    /// The command's keys: the user's, or its defaults.
    func keys(_ command: KeyCommand) -> [String] {
        guard case .array(let values) = preferences[command.preferenceKey] else { return command.defaults }
        return values.compactMap(\.string)
    }

    func keys(_ id: String) -> [String] {
        KeyCommand.named(id).map(keys) ?? []
    }

    /// Whether the user changed the command's keys.
    func keysChanged(_ command: KeyCommand) -> Bool {
        preferences[command.preferenceKey] != nil
    }

    /// The command's first key, as the palette and the tooltips say it. Only the Mac has the keys.
    func keyLabel(_ id: String) -> String? {
        #if os(macOS)
        keys(id).first.map(Keys.label)
        #else
        nil
        #endif
    }

    /// The text with the command's first key after it: "Archive (E)".
    func keyHelp(_ text: String, _ id: String) -> String {
        keyLabel(id).map { "\(text) (\($0))" } ?? text
    }

    /// The first of the command's keys a menu item can show.
    func menuShortcut(_ id: String) -> KeyboardShortcut? {
        keys(id).lazy.compactMap(Keys.menuShortcut).first
    }

    /// The command the key runs where `scope` is: over the list or in a thread.
    func command(for key: String, in scope: KeyScope) -> KeyCommand? {
        KeyCommand.all.first { $0.scope.overlaps(scope) && keys($0).contains(key) }
    }

    /// Whether a key starts a longer one that works where `scope` is, as G starts G I.
    func startsSequence(_ stroke: String, in scope: KeyScope) -> Bool {
        KeyCommand.all.contains { $0.scope.overlaps(scope) && keys($0).contains { $0.hasPrefix(stroke + " ") } }
    }

    /// Puts the keys in place of the command's `replacing` key, or after its keys, and takes them
    /// from the commands that had them where this one works. Says which they were taken from.
    func setKey(_ key: String, for command: KeyCommand, replacing old: String? = nil) {
        var taken: [String] = []
        for other in KeyCommand.all where other != command && other.scope.overlaps(command.scope) {
            let kept = keys(other).filter { !Keys.clash($0, key) }
            guard kept.count != keys(other).count else { continue }
            setKeys(kept, for: other)
            taken.append(other.title)
        }
        var mine = keys(command).filter { $0 != key }
        if let old, let index = mine.firstIndex(of: old) {
            mine[index] = key
        } else {
            mine.append(key)
        }
        setKeys(mine, for: command)
        guard !taken.isEmpty else { return }
        show("\(Keys.label(key)) was taken from \(taken.joined(separator: ", ")).")
    }

    func removeKey(_ key: String, from command: KeyCommand) {
        setKeys(keys(command).filter { $0 != key }, for: command)
    }

    /// Gives the command its defaults back, taking them from the commands that took them since.
    func resetKeys(_ command: KeyCommand) {
        setPreference(command.preferenceKey, nil)
        for key in command.defaults {
            for other in KeyCommand.all where other != command && other.scope.overlaps(command.scope) && keys(other).contains(where: { Keys.clash($0, key) }) {
                setKeys(keys(other).filter { !Keys.clash($0, key) }, for: other)
            }
        }
    }

    func resetAllKeys() {
        for command in KeyCommand.all where keysChanged(command) {
            setPreference(command.preferenceKey, nil)
        }
    }

    private func setKeys(_ keys: [String], for command: KeyCommand) {
        setPreference(command.preferenceKey, keys == command.defaults ? nil : .array(keys.map { .string($0) }))
    }
}
