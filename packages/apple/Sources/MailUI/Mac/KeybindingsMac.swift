#if os(macOS)
import SwiftUI

/// The Keybindings page: every command under its group with its keys, Superhuman's to begin with,
/// to record others, add more, take off, or put back.
struct KeybindingSettings: View {
    @Environment(MailStore.self) private var store
    @State private var query = ""

    var body: some View {
        let changed = KeyCommand.all.contains(where: store.keysChanged)
        HStack(spacing: Space.s) {
            SearchField(
                placeholder: "Search keybindings", text: $query, style: .bar, autofocus: false,
                escape: { if query.isEmpty { store.closeSettings() } }
            )
            ActionButton(title: "Reset all", symbol: .undo, variant: .ghost, action: store.resetAllKeys)
                .disabled(!changed)
                .opacity(Opacity.disabled, when: !changed)
        }
        .padding(.trailing, -Theme.buttonGap / 2)
        .id("keybindings")
        Notice(text: "Click a key to record another in its place, or + to add one. A key given to one command leaves the command that had it.")
            .padding(.horizontal, Space.xxs)
            .padding(.top, -Space.l)
        let found = commands
        ForEach(KeyCommand.groups, id: \.self) { group in
            let rows = found.filter { $0.group == group }
            if !rows.isEmpty {
                SettingsGroup(id: "keys-\(group)", title: group) {
                    ForEach(rows) { command in
                        KeybindingRow(command: command)
                        if command != rows.last { SettingsDivider() }
                    }
                }
            }
        }
        if found.isEmpty {
            SettingsCard { SettingsNote(text: "No keybindings found.") }
        }
    }

    /// The commands every word of the search is found in, by name, group or key.
    private var commands: [KeyCommand] {
        let words = query.split(whereSeparator: \.isWhitespace).map(String.init)
        guard !words.isEmpty else { return KeyCommand.all }
        return KeyCommand.all.filter { command in
            let text = "\(command.title) \(command.group) \(store.keys(command).map(Keys.label).joined(separator: " "))"
            return words.allSatisfy { text.localizedCaseInsensitiveContains($0) }
        }
    }
}

private struct KeybindingRow: View {
    @Environment(MailStore.self) private var store
    let command: KeyCommand

    var body: some View {
        let keys = store.keys(command)
        SettingsRow {
            HStack(spacing: Space.s) {
                Text(command.title).textStyle(.body).lineLimit(1)
                if store.keysChanged(command) {
                    Text("Changed").textStyle(.footnote)
                }
            }
        } trailing: {
            ForEach(keys, id: \.self) { key in
                KeyRecorder(key: key, help: "Record a key in place of \(Keys.label(key))") { store.setKey($0, for: command, replacing: key) }
            }
            KeyRecorder(key: nil, help: "Add a key") { store.setKey($0, for: command) }
            IconMenu(symbol: .ellipsis, help: "More", size: .small, circled: false) {
                Button("Reset to Default") { store.resetKeys(command) }
                    .disabled(!store.keysChanged(command))
                if !keys.isEmpty {
                    Divider()
                    ForEach(keys, id: \.self) { key in
                        Button("Remove \(Keys.label(key))", role: .destructive) { store.removeKey(key, from: command) }
                    }
                }
            }
        }
        .id("key-\(command.id)")
    }
}
#endif
