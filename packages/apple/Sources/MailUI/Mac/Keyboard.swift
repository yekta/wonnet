#if os(macOS)
import AppKit
import SwiftUI

/// The menu bar's commands, with the keys the user gave them that have ⌘ or ⌃: those work while
/// typing too.
struct MacCommands: Commands {
    let store: MailStore

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("New Message") { store.newMessage() }.shortcut("compose", store).disabled(store.sheetOpen)
        }
        CommandGroup(after: .appInfo) {
            Button("Check for Updates…") { store.updater.check(asked: true) }
        }
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { store.openSettings() }.shortcut("settings", store).disabled(store.sheetOpen)
        }
        CommandGroup(replacing: .printItem) {
            Button("Print…") { store.run(.print) }.shortcut("print", store).disabled(store.sheetOpen || store.current == nil)
        }
        CommandMenu("Message") {
            Button("Command Palette…") { store.showPalette() }.shortcut("palette", store).disabled(store.sheetOpen)
            Button("Keybindings") { store.openSettings(.keybindings) }.shortcut("keybindings", store).disabled(store.sheetOpen)
            Divider()
            Button("Unsubscribe") { store.run(.unsubscribe) }.shortcut("unsubscribe", store)
                .disabled(store.sheetOpen || store.current == nil && store.selection.isEmpty)
            Button("Get Me To Zero…") { store.archiveAll() }.shortcut("zero", store).disabled(store.sheetOpen || store.searchRows != nil || store.rows.isEmpty)
        }
        CommandMenu("Go") {
            Button("All Inboxes") { store.select(mailbox: "inbox") }.shortcut("go.account.0", store).disabled(store.sheetOpen)
            ForEach(Array(store.accounts.prefix(9).enumerated()), id: \.element.id) { index, account in
                Button(account.title) { store.select(mailbox: "\(account.id)/inbox") }
                    .shortcut("go.account.\(index + 1)", store)
                    .disabled(store.sheetOpen)
            }
        }
    }
}

extension View {
    /// Gives a menu item the first of the command's keys a menu can show.
    @ViewBuilder func shortcut(_ command: String, _ store: MailStore) -> some View {
        if let shortcut = store.menuShortcut(command) {
            keyboardShortcut(shortcut)
        } else {
            self
        }
    }
}

/// The keys of the window, as the Keybindings settings set them, ignored while typing or with a
/// sheet open. Over the settings, only Esc, to go back.
struct KeyHandler: NSViewRepresentable {
    let store: MailStore
    let focusSearch: () -> Void
    @Binding var sidebarOpen: Bool

    func makeCoordinator() -> KeyCoordinator { KeyCoordinator(store: store) }

    func makeNSView(context: Context) -> NSView {
        let coordinator = context.coordinator
        let view = NSView()
        coordinator.view = view
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            MainActor.assumeIsolated { coordinator.handle(event) } ? nil : event
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.focusSearch = focusSearch
        context.coordinator.sidebarOpen = $sidebarOpen
    }

    static func dismantleNSView(_ view: NSView, coordinator: KeyCoordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
    }
}

@MainActor
final class KeyCoordinator {
    /// A key is being recorded in the settings, so every key is that.
    static var recording = false

    let store: MailStore
    var monitor: Any?
    /// In the window the keys are for: another window's keys (Quick Look's) are left alone.
    weak var view: NSView?
    var focusSearch: () -> Void = {}
    var sidebarOpen: Binding<Bool>?
    /// The first key of a longer one, as G of G I, and when it was pressed.
    private var started: (stroke: String, at: Date)?

    init(store: MailStore) {
        self.store = store
    }

    func handle(_ event: NSEvent) -> Bool {
        guard !Self.recording, let window = view?.window, event.window === window, window.isKeyWindow, window.attachedSheet == nil else { return false }
        guard !(window.firstResponder is NSText) else { return false }
        let plain = event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty
        if store.settings != nil {
            guard plain, event.keyCode == 53, store.confirmation == nil else { return false }
            store.closeSettings()
            return true
        }
        guard !store.sheetOpen, let stroke = KeyNames.stroke(of: event) else { return false }
        if plain, event.keyCode == 53 { return escape() }
        let scope: KeyScope = store.conversation == nil ? .list : .thread
        if let started {
            self.started = nil
            if Date().timeIntervalSince(started.at) < 1.5, let command = store.command(for: "\(started.stroke) \(stroke)", in: scope) {
                return perform(command.id)
            }
        }
        if let command = store.command(for: stroke, in: scope) {
            return perform(command.id)
        }
        guard store.startsSequence(stroke, in: scope) else { return false }
        started = (stroke, Date())
        return true
    }

    /// Does the command, or says it doesn't apply here, so the key goes on to the window.
    private func perform(_ id: String) -> Bool {
        let inThread = store.conversation != nil
        if let command = ThreadCommand(rawValue: id) {
            store.run(command)
            return true
        }
        if id.hasPrefix("go.account.") {
            let number = Int(id.dropFirst("go.account.".count)) ?? 0
            guard number == 0 || number <= store.accounts.count else { return false }
            store.select(mailbox: number == 0 ? "inbox" : "\(store.accounts[number - 1].id)/inbox")
            return true
        }
        if id == "go.label" {
            store.showPalette(.labels)
            return true
        }
        if id.hasPrefix("go.") {
            store.go(to: String(id.dropFirst("go.".count)))
            return true
        }
        switch id {
        case "next": store.move(1)
        case "previous": store.move(-1)
        case "open": return open()
        case "top": store.move(-store.visibleRows.count)
        case "bottom": store.move(store.visibleRows.count)
        case "nextMessage", "previousMessage":
            guard inThread else { return false }
            store.moveMessage(id == "nextMessage" ? 1 : -1)
        case "expandAll":
            guard inThread else { return false }
            store.expandAllMessages()
        case "nextSplit", "previousSplit":
            guard !store.splits.isEmpty, !inThread else { return false }
            store.moveSplit(id == "nextSplit" ? 1 : -1)
        case "search": focusSearch()
        case "palette": store.showPalette()
        case "select": select()
        case "selectNext": store.extendSelection(1)
        case "selectPrevious": store.extendSelection(-1)
        case "selectAll":
            guard !inThread, !store.visibleRows.isEmpty else { return false }
            store.selectAll()
        case "removeLabel": store.removeShownLabel()
        case "undo": store.undo()
        case "compose": store.newMessage()
        case "replyAllInThread":
            guard inThread else { return false }
            store.reply(.replyAll)
        case "unreadOnly": store.toggleFilter(.unread)
        case "starredOnly": store.toggleFilter(.starred)
        case "zero": store.archiveAll()
        case "settings": store.openSettings()
        case "keybindings": store.openSettings(.keybindings)
        default: return false
        }
        return true
    }

    private func open() -> Bool {
        guard store.conversation == nil, let current = store.selected else { return false }
        store.open(current)
        return true
    }

    private func escape() -> Bool {
        if let sidebarOpen, sidebarOpen.wrappedValue {
            withAnimation(Theme.sidebarSlide) { sidebarOpen.wrappedValue = false }
        } else if store.conversation != nil {
            store.close()
        } else if !store.selection.isEmpty {
            store.clearSelection()
        } else if store.searchRows != nil {
            store.endSearch()
        } else if store.filter != nil {
            store.setFilter(nil)
        } else {
            return false
        }
        return true
    }

    /// The row the keyboard is on into the selection, or out of it.
    private func select() {
        guard store.conversation == nil else { return }
        if store.selected == nil { store.move(1) }
        guard let current = store.selected else { return }
        store.toggleSelection(current)
    }
}

/// The names a key press goes by in the keybindings. A letter is the one the keyboard's layout
/// types; a digit or a sign is the key where it is on a US keyboard, so ⇧3 is # in every layout.
enum KeyNames {
    private static let named: [UInt16: String] = [
        36: "enter", 76: "enter", 48: "tab", 49: "space", 51: "backspace", 117: "delete", 53: "escape",
        123: "arrowleft", 124: "arrowright", 125: "arrowdown", 126: "arrowup",
        115: "home", 119: "end", 116: "pageup", 121: "pagedown",
    ]
    private static let signs: [UInt16: String] = [
        29: "0", 18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7", 28: "8", 25: "9",
        27: "-", 24: "=", 33: "[", 30: "]", 42: "\\", 41: ";", 39: "'", 43: ",", 47: ".", 44: "/", 50: "`",
    ]
    private static let letters: [UInt16: String] = [
        0: "a", 11: "b", 8: "c", 2: "d", 14: "e", 3: "f", 5: "g", 4: "h", 34: "i", 38: "j", 40: "k", 37: "l", 46: "m",
        45: "n", 31: "o", 35: "p", 12: "q", 15: "r", 1: "s", 17: "t", 32: "u", 9: "v", 13: "w", 7: "x", 16: "y", 6: "z",
    ]

    /// The key with the modifiers held, as in `mod+shift+k`.
    static func stroke(of event: NSEvent) -> String? {
        guard let name = name(of: event) else { return nil }
        let flags = event.modifierFlags
        let held = [(flags.contains(.command), "mod"), (flags.contains(.control), "ctrl"), (flags.contains(.option), "alt"), (flags.contains(.shift), "shift")]
        return (held.filter(\.0).map(\.1) + [name]).joined(separator: "+")
    }

    private static func name(of event: NSEvent) -> String? {
        if let name = named[event.keyCode] { return name }
        if let typed = event.charactersIgnoringModifiers?.lowercased(), typed.count == 1, let character = typed.first, character.isASCII, character.isLetter {
            return typed
        }
        return signs[event.keyCode] ?? letters[event.keyCode]
    }
}
#endif
