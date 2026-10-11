#if os(macOS)
import AppKit
import SwiftUI

/// The settings over the whole window: their sections on the left, behind a line that drags
/// their width, and the open one's page on the right with its title over it. Back, beside the
/// window's buttons or under the sections, and Esc go back to the mail.
struct SettingsRoute: View {
    @Environment(MailStore.self) private var store
    let section: SettingsSection
    @AppStorage("settings.sidebarWidth") private var sidebarWidth = Double(Theme.sidebarWidth)

    var body: some View {
        let widths = Theme.settingsSidebarWidths
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                // Under the window's buttons and Back, which is the toolbar's.
                Color.clear.frame(height: Theme.barHeight)
                SettingsSidebar(selected: section)
            }
            .frame(width: min(max(CGFloat(sidebarWidth), widths.lowerBound), widths.upperBound))
            PaneDivider(width: $sidebarWidth, widths: widths)
            VStack(spacing: 0) {
                Text(section.title)
                    .textStyle(.labelStrong, color: Tokens.mutedForeground.color)
                    .frame(maxWidth: .infinity)
                    .frame(height: Theme.barHeight)
                    .allowsHitTesting(false)
                SettingsPage(section: section)
            }
        }
        .background(Tokens.background.color)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                ActionButton(title: "Back", symbol: .arrowLeft, variant: .ghost) { store.closeSettings() }
                    .help("Back to the mail (Esc)")
            }
            .withoutSystemGlass()
        }
    }
}

/// The sections under a search for one of them. While something is typed, the groups found take
/// the sections' place.
private struct SettingsSidebar: View {
    @Environment(MailStore.self) private var store
    let selected: SettingsSection

    var body: some View {
        @Bindable var store = store
        VStack(spacing: 0) {
            SearchField(
                placeholder: "Search settings", text: $store.settingsQuery, style: .bar, autofocus: false,
                // Esc clears a search, and with none goes back to the mail.
                escape: { if store.settingsQuery.isEmpty { store.closeSettings() } }
            )
            .padding(.horizontal, Space.m)
            .padding(.vertical, Space.s)
            ScrollView {
                VStack(spacing: 0) {
                    if store.settingsQuery.isEmpty {
                        ForEach(SettingsSection.shown) { section in
                            NavRow(title: section.title, symbol: section.symbol, selected: section == selected, verticalPadding: Theme.buttonGap / 2) {
                                store.openSettings(section)
                            } trailing: {
                                EmptyView()
                            }
                        }
                    } else {
                        results
                    }
                }
                .padding(.vertical, Space.xs)
            }
            NavRow(title: "Back", symbol: .arrowLeft, verticalPadding: 2) { store.closeSettings() } trailing: {
                EmptyView()
            }
            .help("Back to the mail (Esc)")
            .rule(.top)
        }
    }

    @ViewBuilder private var results: some View {
        let found = SettingsEntry.matching(store.settingsQuery)
        ForEach(found) { entry in
            NavRow(title: entry.title, symbol: entry.section.symbol, verticalPadding: Theme.buttonGap / 2) {
                store.openSettings(entry.section, target: entry.id)
            } trailing: {
                Text(entry.section.title).textStyle(.caption)
            }
        }
        if found.isEmpty {
            Text("No settings found").textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, Space.l + 2)
                .padding(.vertical, Space.s)
        }
    }
}

/// The line between the sections and the page, dragged to make the sections wider or narrower.
private struct PaneDivider: View {
    /// How far past the line it can be grabbed.
    private static let reach: CGFloat = 4

    @Binding var width: Double
    let widths: ClosedRange<CGFloat>
    @State private var widthAtStart: Double?

    var body: some View {
        Rule(vertical: true)
            .overlay {
                Color.clear
                    .frame(width: Theme.hairline + 2 * Self.reach)
                    .contentShape(Rectangle())
                    .onHover { inside in
                        if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                    }
                    .gesture(
                        DragGesture(minimumDistance: 1, coordinateSpace: .global)
                            .onChanged { drag in
                                let start = widthAtStart ?? min(max(width, Double(widths.lowerBound)), Double(widths.upperBound))
                                widthAtStart = start
                                width = min(max(start + drag.translation.width, Double(widths.lowerBound)), Double(widths.upperBound))
                            }
                            .onEnded { _ in widthAtStart = nil }
                    )
            }
    }
}

/// A section's page: its groups, as wide as a page grows, scrolled to the group a search picked.
private struct SettingsPage: View {
    @Environment(MailStore.self) private var store
    let section: SettingsSection

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: Space.xxl) {
                    switch section {
                    case .general: GeneralSettings()
                    case .accounts: AccountSettings()
                    case .inbox: InboxSettings()
                    case .reading: ReadingSettings()
                    case .writing: WritingSettings()
                    case .keybindings: KeybindingSettings()
                    }
                }
                .frame(maxWidth: Theme.settingsPageWidth, alignment: .leading)
                .padding(.horizontal, Space.xl)
                .padding(.top, Space.xl)
                .padding(.bottom, Space.xxl * 2)
                .frame(maxWidth: .infinity)
            }
            .fadesTop()
            .id(section)
            .onChange(of: store.settingsTarget, initial: true) {
                guard let target = store.settingsTarget else { return }
                withAnimation { proxy.scrollTo(target, anchor: .top) }
                store.settingsTarget = nil
            }
        }
    }
}

// MARK: General

private struct GeneralSettings: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        @Bindable var store = store
        SettingsGroup(id: "appearance", title: "Appearance") {
            SettingsRow {
                SettingsLabel(title: "Theme")
            } trailing: {
                Segmented(options: Appearance.allCases, selection: $store.appearance, title: \.name)
            }
        }
        SettingsGroup(id: "updates", title: "Updates", detail: "New versions are downloaded and installed on their own; a restart finishes them.") {
            VersionRow()
        }
        SettingsGroup(id: "server", title: "Server", detail: "Wonnet's server syncs your accounts and keeps this Mac's copy of the mail current.") {
            SettingsRow {
                SettingsLabel(title: store.server, detail: store.connectionText, symbol: store.connection == "offline" ? .wifiOff : .server)
            }
        }
    }
}

/// The version this is, the offer of a new one, the update as it goes, and the restart once one
/// is in.
private struct VersionRow: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let updater = store.updater
        SettingsRow {
            SettingsLabel(title: "Wonnet \(updater.current)", detail: status)
        } trailing: {
            switch updater.state {
            case .available: ActionButton(title: "Update", variant: .primary) { updater.install() }
            case .ready: ActionButton(title: "Restart", variant: .primary) { updater.relaunch() }
            case .failed: ActionButton(title: "Try again", symbol: .refreshCw) { updater.retry() }
            default:
                ActionButton(title: "Check for updates", symbol: .refreshCw, pending: updater.state == .checking) { updater.check(asked: true) }
            }
        }
    }

    private var status: String? {
        switch store.updater.state {
        case .idle, .checking: nil
        case .upToDate: "Up to date."
        case .available(let version): "Wonnet \(version) is available."
        case .downloading(let version, let fraction): "Downloading \(version)… \(Int(fraction * 100))%"
        case .installing(let version): "Installing \(version)…"
        case .ready(let version): "Wonnet \(version) is installed. Restart to use it."
        case .failed(let message): message
        }
    }
}

// MARK: Accounts

private struct AccountSettings: View {
    @Environment(MailStore.self) private var store
    @State private var adding = false

    var body: some View {
        let accounts = store.accounts
        VStack(alignment: .leading, spacing: Space.s + 2) {
            SectionHeading(title: "Accounts", detail: "Each account's colour marks its mail in the list and its mailboxes in the sidebar.")
                .padding(.horizontal, Space.xs)
            LazyVStack(spacing: Space.xl) {
                ForEach(accounts) { account in
                    AccountCard(account: account, anchor: account.id == accounts.first?.id ? "signatures" : account.id)
                        .equatable()
                }
            }
            ActionButton(title: "Add account", symbol: .plus, variant: .outline) { adding = true }
        }
        .id("accounts")
        .sheet(isPresented: $adding) {
            AddAccountSheet().environment(store)
        }
        SettingsGroup(id: "sign-out", title: "Wonnet") {
            SettingsRow {
                SettingsLabel(title: "Sign out", detail: "Every account leaves this Mac. Signing in again brings them back.")
            } trailing: {
                ActionButton(title: "Sign out", symbol: .logOut, variant: .destructive) { store.confirmSigningOut() }
            }
        }
    }
}

/// An account's card. The account changes with its mailboxes' counts as mail syncs, which the
/// card doesn't show, so it is drawn again only when what it shows changes.
private struct AccountCard: View, Equatable {
    @Environment(MailStore.self) private var store
    let account: AccountView
    /// What a search scrolls to: the first card's signature is the settings' "signatures".
    let anchor: String

    static func == (lhs: Self, rhs: Self) -> Bool {
        let a = lhs.account, b = rhs.account
        return lhs.anchor == rhs.anchor && a.id == b.id && a.address == b.address && a.name == b.name && a.title == b.title
            && a.color == b.color && a.status == b.status && a.identities == b.identities
    }

    var body: some View {
        SettingsCard {
            SettingsRow {
                SettingsLabel(title: account.title, detail: store.subtitle(of: account), dot: Theme.accountColor(account.color).color, heading: true)
            } trailing: {
                IconMenu(symbol: .ellipsis, help: "More", circled: false) {
                    Button(role: .destructive) { store.confirmRemoving(account) } label: { Text("Remove Account…") }
                }
            }
            SettingsDivider()
            SettingsRow {
                SettingsLabel(title: "Name", detail: "Shown in the sidebar in place of the address.")
            } trailing: {
                AccountNameField(account: account)
            }
            SettingsDivider()
            SettingsRow {
                SettingsLabel(title: "Colour")
            } trailing: {
                SwatchRow(selected: account.color) { store.setAccountColor(account.id, $0) }
            }
            SettingsDivider()
            SettingsRow {
                SettingsLabel(title: "Notifications", detail: "New mail makes a notification and counts on the badge.")
            } trailing: {
                Switch(title: "Notifications", isOn: store.notifies(account))
            }
            SettingsDivider()
            SignatureRow(account: account)
                .id(anchor)
        }
    }
}

/// The signature put under what the account sends, and Edit, which opens it in a box to change.
private struct SignatureRow: View {
    @Environment(MailStore.self) private var store
    let account: AccountView
    /// The text being written, while the box is open.
    @State private var draft: String?

    var body: some View {
        let own = store.ownSignature(account)
        let provided = store.providedSignature(account)
        let shown = own.isEmpty ? provided : own
        VStack(alignment: .leading, spacing: 0) {
            SettingsRow {
                SettingsLabel(title: "Signature", detail: draft == nil ? summary(own: own, provided: provided) : nil)
            } trailing: {
                if draft == nil {
                    ActionButton(title: shown == nil ? "Add" : "Edit", symbol: shown == nil ? .plus : .pencil, variant: .ghost) {
                        draft = shown ?? ""
                    }
                }
            }
            if let draft {
                editor(draft, start: shown ?? "", provided: provided)
            } else if let shown {
                Text(shown)
                    .textStyle(.label, color: Tokens.mutedForeground.color)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Space.m)
                    .background(RoundedRectangle(cornerRadius: Radius.sm).fill(Tokens.backgroundSecondary.color))
                    .overlay(RoundedRectangle(cornerRadius: Radius.sm).strokeBorder(Tokens.borderCard.color, lineWidth: Theme.hairline))
                    .padding([.horizontal, .bottom], Space.l)
            }
        }
    }

    private func editor(_ text: String, start: String, provided: String?) -> some View {
        VStack(alignment: .leading, spacing: Space.s) {
            InputField(text: Binding(get: { draft ?? "" }, set: { draft = $0 }), placeholder: "Best,\n\(account.address)", lines: 5, autofocus: true)
            if provided != nil {
                Notice(text: "Left empty, the one kept by \(store.providerName(account)) is used.")
            }
            HStack(spacing: 0) {
                Spacer(minLength: 0)
                FormButtons(saveDisabled: text == start) {
                    store.setSignature(text.trimmingCharacters(in: .whitespacesAndNewlines), for: account)
                    draft = nil
                } onCancel: {
                    draft = nil
                }
            }
        }
        .padding([.horizontal, .bottom], Space.l)
    }

    private func summary(own: String, provided: String?) -> String? {
        guard own.isEmpty else { return nil }
        guard provided != nil else { return "None. It goes under what you write from this account." }
        return "Kept by \(store.providerName(account)), until you write your own."
    }
}

/// Gmail, or a JMAP server; it closes once the account is in.
private struct AddAccountSheet: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Sheet(title: "Add account", size: .small, background: Tokens.background) {
            ScrollView {
                AddAccountForm().padding(Space.xl)
            }
        }
        .onChange(of: store.accounts.count) { dismiss() }
    }
}

// MARK: Inbox

private struct InboxSettings: View {
    @Environment(MailStore.self) private var store
    /// The split being written, new or not.
    @State private var editing: CustomSplit?

    var body: some View {
        let splits = store.customSplits
        SettingsGroup(id: "split-inbox", title: "Split Inbox", detail: "Your splits come after Important. Drag one to move it.") {
            SettingsRow {
                SettingsLabel(title: "Split Inbox", detail: "Important mail first, then your splits, then Other: newsletters and notifications.")
            } trailing: {
                Switch(title: "Split Inbox", isOn: store.splitInboxOn)
            }
            SettingsDivider()
            MovableList(
                items: splits, scrolls: false, group: { _ in "splits" }, clicked: { editing = $0 }, moved: store.moveSplit,
                menu: { split in
                    [RowMenuItem(title: "Edit") { editing = split }, RowMenuItem(title: "Remove", destructive: true) { store.removeSplit(split) }]
                }
            ) { split in
                MovableRow {
                    SettingsLabel(title: split.name, detail: store.summary(of: split), symbol: .listFilter)
                } trailing: {
                    Image(.chevronRight, size: 12).foregroundStyle(Tokens.mutedStrongerForeground.color)
                }
            }
            SettingsRow {
                ActionButton(title: "Add split", symbol: .plus, variant: .outline) { editing = store.newSplit() }
            }
        }
        .sheet(item: $editing) { split in
            SplitSheet(split: split).environment(store)
        }
        SettingsGroup(id: "tabs", title: "Tabs", detail: "The mailboxes kept over the list. Drag a tab to move it; right-click it to remove it.") {
            TabRows()
        }
    }
}

private struct TabRows: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let tabs = store.savedTabs
        if tabs.isEmpty {
            SettingsNote(text: "No tabs. Mailboxes are opened from the sidebar.")
            SettingsDivider()
        }
        MovableList(
            items: tabs, scrolls: false, group: { _ in "tabs" }, moved: store.moveTab,
            menu: { tab in [RowMenuItem(title: "Remove Tab", destructive: true) { store.toggleTab(tab.id) }] }
        ) { tab in
            MovableRow {
                SettingsLabel(title: store.tabName(tab), symbol: Symbol.named(tab.symbol))
            } trailing: {
                EmptyView()
            }
        }
        let left = store.mailboxesForTabs
        SettingsRow {
            ActionMenu(title: "Add tab", symbol: .plus) {
                ForEach(left.unified) { mailbox in item(mailbox) }
                ForEach(left.accounts, id: \.account.id) { account, mailboxes in
                    Menu(account.title) {
                        ForEach(mailboxes) { mailbox in item(mailbox) }
                    }
                }
            }
        }
    }

    private func item(_ mailbox: Mailbox) -> some View {
        Button { store.toggleTab(mailbox.id) } label: {
            Label { Text(mailbox.name) } icon: { Image(Symbol.named(mailbox.symbol), size: 15) }
        }
    }
}

/// A row a `MovableList` moves inside a settings card: it hovers, and keeps the card's hairline
/// under it, over the row that adds one.
private struct MovableRow<Leading: View, Trailing: View>: View {
    @ViewBuilder let leading: Leading
    @ViewBuilder let trailing: Trailing
    @Environment(\.hovered) private var hovered

    var body: some View {
        SettingsRow { leading } trailing: { trailing }
            .background(hovered ? Tokens.backgroundAccentLarger.color : .clear)
            .rule(.bottom, color: Tokens.borderCard)
    }
}

private struct SplitSheet: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State var split: CustomSplit
    @State private var senders = ""

    var body: some View {
        let isNew = !store.customSplits.contains { $0.id == split.id }
        let labels = store.splitLabels
        Sheet(title: isNew ? "New split" : "Edit split", size: .small, background: Tokens.background) {
            VStack(alignment: .leading, spacing: Space.m) {
                InputField(label: "Name", text: $split.name, placeholder: "Team", autofocus: true)
                InputField(label: "From", text: $senders, placeholder: "ann@example.com, @example.com")
                Notice(text: "Addresses, or @ and a domain for everyone there.")
                if !labels.isEmpty {
                    Dropdown(label: "With the label", options: labels.map(\.id), selection: $split.label, none: "None") { id in
                        labels.first { $0.id == id }?.name ?? id
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(Space.xl)
        } trailing: {
            ActionButton(title: "Save", variant: .primary, action: save)
                .keyboardShortcut(.defaultAction)
                .disabled(split.name.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .onAppear { senders = split.from.joined(separator: ", ") }
    }

    private func save() {
        split.name = split.name.trimmingCharacters(in: .whitespaces)
        split.from = CustomSplit.senders(senders)
        store.saveSplit(split)
        dismiss()
    }
}

// MARK: Reading

private struct ReadingSettings: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        SettingsGroup(id: "mail", title: "Mail") {
            SettingsRow {
                SettingsLabel(
                    title: "Dark mode for mail",
                    detail: "In dark mode, newsletters and other designed mail are drawn in dark colours. Each message can show the original."
                )
            } trailing: {
                Switch(title: "Dark mode for mail", isOn: store.flag("dark_mail", default: true))
            }
            SettingsDivider()
            SettingsRow {
                SettingsLabel(
                    title: "Load remote images",
                    detail: "Off, pictures hosted by the sender wait until you ask for them, so nobody learns when you read their mail."
                )
            } trailing: {
                Switch(title: "Load remote images", isOn: store.flag("remote_images", default: true))
            }
        }
        SettingsGroup(id: "blocked", title: "Blocked senders", detail: "Their new mail goes to the trash. Block someone from a message of theirs.") {
            let blocked = store.blockedSenders
            if blocked.isEmpty {
                SettingsNote(text: "No one is blocked.")
            }
            ForEach(blocked, id: \.self) { address in
                SettingsRow {
                    SettingsLabel(title: address, symbol: .ban)
                } trailing: {
                    ActionButton(title: "Unblock", variant: .ghost) { store.unblock(address) }
                }
                if address != blocked.last { SettingsDivider() }
            }
        }
    }
}

// MARK: Writing

private struct WritingSettings: View {
    @Environment(MailStore.self) private var store
    /// The snippet being written, new or not.
    @State private var editing: Snippet?

    var body: some View {
        @Bindable var store = store
        SettingsGroup(id: "sending", title: "Sending") {
            SettingsRow {
                SettingsLabel(title: "Undo send", detail: "How long a message waits after Send, to be taken back.")
            } trailing: {
                Segmented(options: MailStore.undoDelays, selection: $store.undoDelay, title: MailStore.undoDelayName)
            }
        }
        let snippets = store.snippets
        SettingsGroup(
            id: "snippets", title: "Snippets",
            detail: "Type ; and a name while writing to put one in, or press ⌘;. {first_name} becomes the first recipient's first name."
        ) {
            ForEach(snippets) { snippet in
                SettingsRow {
                    SettingsLabel(title: snippet.name, detail: snippet.text.replacingOccurrences(of: "\n", with: " "), detailLines: 1)
                } trailing: {
                    IconMenu(symbol: .ellipsis, help: "More", circled: false) {
                        Button("Edit…") { editing = snippet }
                        Button("Remove", role: .destructive) { store.setPreference(snippet.key, nil) }
                    }
                }
                SettingsDivider()
            }
            SettingsRow {
                ActionButton(title: "Add snippet", symbol: .plus, variant: .outline) {
                    editing = Snippet(id: UUID().uuidString.lowercased(), name: "", text: "")
                }
            }
        }
        .sheet(item: $editing) { snippet in
            SnippetSheet(snippet: snippet).environment(store)
        }
    }
}

private struct SnippetSheet: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State var snippet: Snippet

    var body: some View {
        let isNew = !store.snippets.contains { $0.id == snippet.id }
        Sheet(title: isNew ? "New snippet" : "Edit snippet", size: .small, background: Tokens.background) {
            VStack(alignment: .leading, spacing: Space.m) {
                InputField(label: "Name", text: $snippet.name, placeholder: "thanks", autofocus: true)
                InputField(label: "Text", text: $snippet.text, placeholder: "Thanks {first_name}!", lines: 8)
                Spacer(minLength: 0)
            }
            .padding(Space.xl)
        } trailing: {
            ActionButton(title: "Save", variant: .primary) {
                store.saveSnippet(snippet)
                dismiss()
            }
            .keyboardShortcut(.defaultAction)
            .disabled(snippet.name.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }
}
#endif
