import SwiftUI

/// Every component, in every state, on one page: what the apps are made of, to look at while
/// working on them. Opened from the palette.
struct GalleryView: View {
    @State private var text = "Typed"
    @State private var empty = ""
    @State private var query = ""
    @State private var on = true
    @State private var off = false
    @State private var theme = "Light"
    @State private var choice: String? = "Work"
    @State private var person = "ada"
    @State private var tab = "important"
    @State private var swatch = "account-3"

    private struct Tab: Identifiable, Hashable {
        let id: String
        let name: String
        let count: Int
    }

    private let people = [
        Choice(id: "ada", title: "Ada Lovelace", detail: "ada@example.com", dot: Tokens.account1.color, keywords: "Work"),
        Choice(id: "sam", title: "sam@acme.example", detail: "Acme", dot: Tokens.account2.color),
        Choice(id: "grace", title: "Grace Hopper", detail: "grace@example.com", dot: Tokens.account3.color, keywords: "Home"),
        Choice(id: "alan", title: "alan@example.com", detail: "Home", dot: Tokens.account3.color),
    ]

    var body: some View {
        Sheet(title: "Components", size: .large, background: Tokens.background) {
            ScrollView {
                VStack(alignment: .leading, spacing: Space.xxl) {
                    FormSection(title: "Text", detail: "The scale: a view picks a style, never a size.") {
                        Text("Display").textStyle(.display)
                        Text("Title").textStyle(.title)
                        Text("Heading").textStyle(.heading)
                        Text("Subheading").textStyle(.subheading)
                        Text("Body").textStyle(.body)
                        Text("Body strong").textStyle(.bodyStrong)
                        Text("Label").textStyle(.label)
                        Text("Label strong").textStyle(.labelStrong)
                        Text("Caption").textStyle(.caption)
                        Text("Footnote").textStyle(.footnote)
                        Text("Footnote strong").textStyle(.footnoteStrong)
                        Text("OVERLINE").textStyle(.overline)
                        Text("⌘K  mono").textStyle(.mono)
                    }
                    FormSection(title: "Buttons", detail: "Four variants, three sizes, an icon and a pending state on each.") {
                        HStack(spacing: 0) {
                            ActionButton(title: "Primary", symbol: .send, variant: .primary) {}
                            ActionButton(title: "Outline", symbol: .squarePen, variant: .outline) {}
                            ActionButton(title: "Icon only", symbol: .squarePen, variant: .outline, iconOnly: true) {}
                            ActionButton(title: "Ghost", variant: .ghost) {}
                            ActionButton(title: "Destructive", symbol: .trash, variant: .destructive) {}
                        }
                        HStack(spacing: 0) {
                            ActionButton(title: "Small", symbol: .plus, size: .small) {}
                            ActionButton(title: "Regular", symbol: .plus) {}
                            ActionButton(title: "Large", symbol: .plus, size: .large) {}
                            ActionButton(title: "Pending", symbol: .refreshCw, variant: .primary, pending: true) {}
                            ActionButton(title: "Disabled", symbol: .plus) {}.disabled(true)
                        }
                        HStack(spacing: 0) {
                            IconButton(symbol: .archive, help: "Archive") {}
                            IconButton(symbol: .trash, help: "Trash") {}
                            IconButton(symbol: .clock, help: "Snooze", size: .small) {}
                            IconButton(symbol: .tag, help: "Label", size: .large) {}
                            IconButton(symbol: .alarmClock, help: "Active", active: true) {}
                            IconButton(symbol: .sun, help: "Quiet", circled: false, quiet: true) {}
                            IconButton(symbol: .refreshCw, help: "Pending", pending: true) {}
                            IconButton(symbol: .x, help: "Disabled") {}.disabled(true)
                            FloatingButton(symbol: .squarePen, help: "Compose") {}
                            FloatingButton(symbol: .arrowUp, help: "Back to Top", small: true) {}
                            IconMenu(symbol: .ellipsis, help: "More") {
                                Button("One") {}
                                Button("Two") {}
                            }
                        }
                        HStack(spacing: 0) {
                            ActionMenu(title: "Menu", symbol: .plus) {
                                Button("One") {}
                                Button("Two") {}
                            }
                            ActionMenu(title: "Ghost menu", variant: .ghost) {
                                Button("One") {}
                            }
                        }
                        HStack(spacing: 0) {
                            StarButton(starred: true) {}
                            StarButton(starred: false) {}
                            PlainButton(action: {}) { Text("Undo").textStyle(.labelStrong, color: Tokens.primary.color) }
                        }
                        HStack(spacing: Space.s) {
                            GlassBar {
                                IconButton(symbol: .archive, help: "Archive", circled: false, strong: true) {}
                                IconButton(symbol: .trash, help: "Trash", circled: false, strong: true) {}
                                IconButton(symbol: .clock, help: "Snooze", circled: false, strong: true) {}
                                IconMenu(symbol: .ellipsis, help: "More", circled: false, strong: true) {
                                    Button("One") {}
                                }
                            }
                            GlassCircle(symbol: .reply, help: "Reply") {}
                        }
                    }
                    FormSection(title: "Fields") {
                        InputField(label: "Name", text: $text, placeholder: "Placeholder")
                        InputField(label: "Empty", text: $empty, placeholder: "Placeholder")
                        InputField(label: "Password", text: $text, secure: true)
                        InputField(label: "Lines", text: $empty, placeholder: "A few lines", lines: 3)
                        Dropdown(label: "Dropdown", options: ["Work", "Home"], selection: $choice, none: "None") { $0 }
                        Combobox(title: "Send as", placeholder: "Find an address", choices: people, selected: person, label: "Combobox") {
                            person = $0.id
                        }
                        ChoiceMenu(options: ["System", "Light", "Dark"], selection: $theme) { $0 }
                        SearchField(placeholder: "Search", text: $query, style: .bar, autofocus: false)
                        Card { SearchField(placeholder: "In a sheet", text: $query, autofocus: false) }
                        ToggleRow(title: "On", detail: "With a line about it.", isOn: $on)
                        ToggleRow(title: "Off", isOn: $off)
                        Disclosure(title: "Advanced") { Notice(text: "What was folded.") }
                    }
                    FormSection(title: "Chips") {
                        FlowLayout {
                            Chip(title: "Label", symbol: .tag)
                            Chip(title: "Ada Lovelace", remove: {})
                            Chip(title: "not-an-address", variant: .invalid, remove: {})
                            Chip(title: "report.pdf", symbol: .paperclip, detail: "2 MB", action: {})
                            Chip(title: "Downloading", symbol: .paperclip, pending: true)
                            Chip(title: "Unread", symbol: .listFilter, remove: {})
                        }
                    }
                    FormSection(title: "Rows") {
                        ItemRow(title: "An item", detail: "With a detail and controls", dot: Tokens.account3.color) {
                            IconButton(symbol: .pencil, help: "Edit") {}
                            IconButton(symbol: .trash, help: "Remove") {}
                        }
                        ItemRow(title: "blocked@example.com", symbol: .ban) {
                            ActionButton(title: "Unblock", variant: .ghost) {}
                        }
                        Card {
                            VStack(spacing: 0) {
                                NavRow(title: "Inbox", symbol: .inbox, count: 44, selected: true) {}
                                NavRow(title: "Starred", symbol: .star, count: 8) {}
                                NavRow(title: "isaac@example.com", dot: Tokens.account1.color) {} trailing: {
                                    Image(.chevronRight, size: 12).foregroundStyle(Tokens.mutedStrongestForeground.color)
                                }
                            }
                            .padding(-Space.l)
                        }
                        PopupCard {
                            ChoiceRow(title: "Highlighted", detail: "Detail", symbol: .clock, highlighted: true) {}
                            ChoiceRow(title: "A choice", detail: "⌘K", symbol: .command) {}
                            ChoiceRow(title: "Ada Lovelace", detail: "ada@example.com", avatar: Address(name: "Ada Lovelace", email: "ada@example.com")) {}
                            ChoiceRow(title: "Sam Lee", detail: "sam@acme.example", dot: Tokens.account2.color, checked: true) {}
                            ChoiceRow(title: "Plain") {}
                        }
                        TabStrip(
                            tabs: [Tab(id: "important", name: "Important", count: 3), Tab(id: "other", name: "Other", count: 0)],
                            selected: tab, title: \.name, count: \.count, pick: { tab = $0.id }
                        )
                        .padding(.horizontal, Space.l)
                        .rule(.bottom)
                        PillTabs(
                            tabs: [Tab(id: "important", name: "Inbox", count: 12), Tab(id: "other", name: "Starred", count: 0)],
                            selected: tab, symbol: { $0.id == "important" ? .inbox : .star }, title: \.name, count: \.count,
                            pick: { tab = $0.id }, reorder: { _ in }, menu: { _ in [RowMenuItem(title: "Remove from Tabs") {}] }
                        )
                        BottomTabs(
                            tabs: [Tab(id: "important", name: "Inbox", count: 12), Tab(id: "other", name: "Starred", count: 0)],
                            selected: tab, symbol: { $0.id == "important" ? .inbox : .star }, title: \.name, count: \.count,
                            pick: { tab = $0.id }, reorder: { _ in }
                        )
                    }
                    FormSection(title: "Settings", detail: "The Mac's settings pages; iOS's are the system's grouped lists.") {
                        SettingsGroup(id: "gallery", title: "A group", detail: "With a line about it.") {
                            SettingsRow {
                                SettingsLabel(title: "Work", detail: "isaac@example.com · Gmail", dot: Tokens.account1.color, heading: true)
                            }
                            SettingsDivider()
                            SettingsRow {
                                SettingsLabel(title: "A field", detail: "What it names.")
                            } trailing: {
                                InputField(text: $empty, placeholder: "Placeholder").frame(width: Theme.settingsFieldWidth)
                            }
                            SettingsDivider()
                            SettingsRow {
                                SettingsLabel(title: "A switch", detail: "What it turns on.")
                            } trailing: {
                                Switch(title: "A switch", isOn: $on)
                            }
                            SettingsDivider()
                            SettingsRow {
                                SettingsLabel(title: "isaac@example.com", detail: "Gmail", dot: Tokens.account1.color)
                            } trailing: {
                                KeyCaps(caps: Keys.caps("g shift+3"))
                                #if os(macOS)
                                KeyRecorder(key: "mod+k") { _ in }
                                KeyRecorder(key: nil) { _ in }
                                #endif
                            }
                            SettingsDivider()
                            SettingsNote(text: "Nothing set yet.")
                        }
                    }
                    FormSection(title: "Notes") {
                        Notice(text: "A line about something.")
                        Notice(text: "Something went wrong: the server said no.", tone: .error)
                        Notice(text: "Images from the sender were left out.", action: "Load images")
                        Banner(symbol: .circleCheck, text: "Wonnet 2.0 is installed. Restart to use it.", action: "Restart") {}
                        Card { EmptyState(symbol: .inbox, message: "All done. Enjoy the quiet.").frame(height: 120) }
                        HStack(spacing: Space.m) {
                            Avatar(initials: "AL", email: "ada@example.com", size: 52)
                            Avatar(initials: "GH", email: "grace@example.com", size: 34)
                            Avatar(initials: "K", email: "k@example.com")
                        }
                        SwatchRow(selected: swatch) { swatch = $0 }
                    }
                }
                .padding(Space.xxl)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}
