#if os(iOS)
import SwiftUI

/// The settings in a sheet: a narrow one lists the sections and pushes the open one over the list,
/// a wide one (an iPad's) has the list beside it. Pages are the system's grouped lists, with its
/// switches, menus and pages pushed for what is written.
struct SettingsSheet: View {
    /// A sheet at least this wide has the sections beside the open one.
    private static let wide: CGFloat = 700

    @Environment(MailStore.self) private var store
    @State private var path: [SettingsSection] = []

    var body: some View {
        GeometryReader { sheet in
            let wide = sheet.size.width >= Self.wide
            Group {
                if wide {
                    split
                } else {
                    stack
                }
            }
            .onAppear { follow(wide: wide) }
            .onChange(of: store.settingsTarget) { follow(wide: wide) }
            .onChange(of: wide) { _, wide in follow(wide: wide) }
        }
        .tint(Tokens.primary.color)
        .presentationBackground(Tokens.background.color)
        .presentationSizing(.page)
        .presentationDragIndicator(.visible)
    }

    private var stack: some View {
        NavigationStack(path: $path) {
            SettingsSections { path = [$0] }
                .navigationTitle("Settings")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { SheetCloseItem { store.closeSettings() } }
                .navigationDestination(for: SettingsSection.self) { section in
                    SettingsPage(section: section)
                }
        }
        .onChange(of: path) {
            guard let section = path.last, section != store.settings else { return }
            store.settings = section
        }
    }

    private var split: some View {
        NavigationSplitView(columnVisibility: .constant(.all)) {
            SettingsSections(selected: store.settings ?? .general) { store.settings = $0 }
                .navigationTitle("Settings")
                .navigationBarTitleDisplayMode(.inline)
                .navigationSplitViewColumnWidth(Theme.sidebarWidth)
                .toolbar(removing: .sidebarToggle)
        } detail: {
            NavigationStack {
                SettingsPage(section: store.settings ?? .general)
                    .toolbar { SheetCloseItem { store.closeSettings() } }
            }
            .id(store.settings)
        }
        .navigationSplitViewStyle(.balanced)
    }

    /// A narrow sheet opens the section the store was pointed at, unless that is only the first.
    private func follow(wide: Bool) {
        guard !wide else { return path = [] }
        guard let section = store.settings, section != .general || store.settingsTarget != nil, path.last != section else { return }
        path = [section]
    }
}

/// The sections in groups, each pushing its page, or chosen beside it on a wide sheet. While a
/// search is typed, the groups it finds take their place.
private struct SettingsSections: View {
    private static let groups: [[SettingsSection]] = [[.general, .accounts], [.inbox, .reading, .writing], [.connected]]

    @Environment(MailStore.self) private var store
    /// The section shown beside the list, on a wide sheet. Without one, each pushes.
    var selected: SettingsSection?
    let choose: (SettingsSection) -> Void

    var body: some View {
        @Bindable var store = store
        SettingsList {
            if store.settingsQuery.isEmpty {
                ForEach(Self.groups, id: \.self) { group in
                    Section {
                        ForEach(group) { section in
                            if let selected {
                                Button { choose(section) } label: { row(section) }
                                    .listRowBackground(selected == section ? Tokens.backgroundAccentStronger.color : Tokens.card.color)
                            } else {
                                NavigationLink(value: section) { row(section) }
                            }
                        }
                    }
                }
            } else {
                results
            }
        }
        .searchField(text: $store.settingsQuery, prompt: "Search")
    }

    private func row(_ section: SettingsSection) -> some View {
        Label {
            Text(section.title).foregroundStyle(Tokens.foreground.color)
        } icon: {
            SettingsTile(symbol: section.symbol)
        }
    }

    @ViewBuilder private var results: some View {
        let found = SettingsEntry.matching(store.settingsQuery)
        Section {
            if found.isEmpty {
                Text("No settings found").foregroundStyle(Tokens.mutedForeground.color)
            }
            ForEach(found) { entry in
                Button {
                    store.openSettings(entry.section, target: entry.id)
                    choose(entry.section)
                } label: {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(entry.title).foregroundStyle(Tokens.foreground.color)
                            Text(entry.section.title).textStyle(.caption, color: Tokens.mutedForeground.color)
                        }
                    } icon: {
                        SettingsTile(symbol: entry.section.symbol)
                    }
                }
            }
        }
    }
}

/// One section's page, scrolled to the group a search picked.
private struct SettingsPage: View {
    @Environment(MailStore.self) private var store
    let section: SettingsSection

    var body: some View {
        ScrollViewReader { proxy in
            SettingsList {
                switch section {
                case .general: GeneralPage()
                case .accounts: AccountsPage()
                case .connected: ConnectedAppsPage()
                case .inbox: InboxPage()
                case .reading: ReadingPage()
                case .writing: WritingPage()
                case .keybindings: EmptyView()
                }
            }
            .onChange(of: store.settingsTarget, initial: true) {
                guard let target = store.settingsTarget else { return }
                withAnimation { proxy.scrollTo(target, anchor: .top) }
                store.settingsTarget = nil
            }
        }
        .navigationTitle(section.title)
        .navigationBarTitleDisplayMode(.inline)
    }
}

// MARK: General

private struct GeneralPage: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        @Bindable var store = store
        SettingsListSection("Appearance", footer: Platform.pad ? "Open mail next to the list when the screen is wide enough." : nil) {
            Picker("Theme", selection: $store.appearance) {
                ForEach(Appearance.allCases) { Text($0.name).tag($0) }
            }
            if Platform.pad {
                Toggle("Reading Pane", isOn: $store.readingPane)
            }
        }
        .id("appearance")
        SettingsListSection("Notifications", footer: "The button a new mail's notification has next to Mark as Read and Reply. After it, the notification offers Undo.") {
            Picker("Button", selection: $store.notificationAction) {
                ForEach(NotificationAction.allCases) { Text($0.name).tag($0) }
            }
        }
        .id("notifications")
        SettingsListSection("About") {
            LabeledContent("Version", value: Platform.version)
        }
        .id("updates")
        SettingsListSection("Server", footer: "Wonnet's server syncs your accounts and keeps this \(Platform.device)'s copy of the mail current. Offline, changes wait and are sent once connected.") {
            LabeledContent("Address", value: store.server)
            LabeledContent("Status", value: store.connection == "online" ? "Connected" : store.connection == "connecting" ? "Connecting" : "Offline")
        }
        .id("server")
    }
}

// MARK: Accounts

private struct AccountsPage: View {
    @Environment(MailStore.self) private var store
    @State private var signingOut = false

    var body: some View {
        SettingsListSection("Accounts", footer: "Each account's colour marks its mail in the list and its mailboxes in the sidebar.") {
            NavigationLink {
                AddAccountPage()
            } label: {
                SettingsActionLabel(title: "Add Account", symbol: .plus)
            }
            ForEach(store.accounts) { account in
                NavigationLink {
                    AccountPage(id: account.id)
                } label: {
                    SettingsItem(title: account.title, detail: store.subtitle(of: account)) {
                        Circle().fill(Theme.accountColor(account.color).color).frame(width: 10, height: 10)
                            .frame(width: SettingsTile.size, height: SettingsTile.size)
                    }
                }
            }
        }
        .id("accounts")
        SettingsListSection(footer: "Every account leaves this \(Platform.device). Signing in again brings them back.") {
            Button("Sign Out", role: .destructive) { signingOut = true }
                .foregroundStyle(Tokens.destructive.color)
        }
        .id("sign-out")
        .confirmationDialog("Sign out of Wonnet?", isPresented: $signingOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) {
                store.closeSettings()
                store.signOut()
            }
        }
    }
}

/// An account: what it is, its name, its colour, its notifications, its signature, and the way to
/// remove it. It goes back once the account is gone.
private struct AccountPage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var removing = false

    var body: some View {
        SettingsList {
            if let account = store.accounts.first(where: { $0.id == id }) {
                SettingsListSection(footer: "Shown in the sidebar in place of the address.") {
                    LabeledContent("Name") { AccountNameField(account: account) }
                    LabeledContent("Address", value: account.address)
                    LabeledContent("Status", value: store.status(of: account))
                }
                SettingsListSection("Colour") {
                    SwatchRow(selected: account.color) { store.setAccountColor(account.id, $0) }
                }
                SettingsListSection(footer: "New mail makes a notification and counts on the badge.") {
                    Toggle("Notifications", isOn: store.notifies(account))
                }
                SignatureSection(account: account)
                Section {
                    Button("Remove Account", role: .destructive) { removing = true }
                        .foregroundStyle(Tokens.destructive.color)
                }
                .confirmationDialog("Remove \(account.address)?", isPresented: $removing, titleVisibility: .visible) {
                    Button("Remove Account", role: .destructive) { store.removeAccount(account.id) }
                } message: {
                    Text("Its mail leaves this \(Platform.device) and your other devices. You can add it again.")
                }
            }
        }
        .navigationTitle(store.accounts.first(where: { $0.id == id })?.title ?? "Account")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: store.accounts.contains { $0.id == id }) { _, there in
            if !there { dismiss() }
        }
    }
}

/// The signature put under what the account sends, and Edit, which opens it in a box to change.
private struct SignatureSection: View {
    @Environment(MailStore.self) private var store
    let account: AccountView
    /// The text being written, while the box is open.
    @State private var draft: String?
    @FocusState private var focused: Bool

    var body: some View {
        let own = store.ownSignature(account)
        let provided = store.providedSignature(account)
        let shown = own.isEmpty ? provided : own
        SettingsListSection("Signature", footer: footer(own: own, provided: provided)) {
            if let draft {
                TextField("Best,\n\(account.address)", text: Binding(get: { self.draft ?? "" }, set: { self.draft = $0 }), axis: .vertical)
                    .lineLimit(4...12)
                    .focused($focused)
                    .onAppear { focused = true }
                HStack(spacing: 0) {
                    Button("Cancel") { self.draft = nil }
                        .buttonStyle(.borderless)
                    Spacer(minLength: 0)
                    Button("Save") {
                        store.setSignature(draft.trimmingCharacters(in: .whitespacesAndNewlines), for: account)
                        self.draft = nil
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(draft == (shown ?? ""))
                }
            } else {
                if let shown {
                    Text(shown).foregroundStyle(Tokens.mutedForeground.color).textSelection(.enabled)
                } else {
                    Text("No signature").foregroundStyle(Tokens.mutedStrongerForeground.color)
                }
                Button { self.draft = shown ?? "" } label: {
                    SettingsActionLabel(title: shown == nil ? "Add Signature" : "Edit Signature", symbol: shown == nil ? .plus : .pencil)
                }
            }
        }
    }

    private func footer(own: String, provided: String?) -> String {
        guard provided != nil, own.isEmpty || draft != nil else { return "Put under what you write from this account." }
        guard draft == nil else { return "Left empty, the one kept by \(store.providerName(account)) is used." }
        return "Kept by \(store.providerName(account)), until you write your own."
    }
}

/// Gmail, Outlook, or a JMAP server; it goes back once the account is in.
private struct AddAccountPage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        AddAccountFlow()
            .navigationTitle("Add Account")
            .navigationBarTitleDisplayMode(.inline)
            .onChange(of: store.accounts.count) { dismiss() }
    }
}

// MARK: Inbox

private struct InboxPage: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let splits = store.customSplits
        SettingsListSection(footer: "Important mail first, then your splits, then Other: newsletters and notifications.") {
            Toggle("Split Inbox", isOn: store.splitInboxOn)
        }
        .id("split-inbox")
        SettingsListSection("Splits", footer: "Hold a split and drag it to move it; swipe it to remove it.") {
            ForEach(splits) { split in
                NavigationLink {
                    SplitPage(split: split)
                } label: {
                    SettingsItem(title: split.name, detail: store.summary(of: split)) { SettingsTile(symbol: .listFilter) }
                }
            }
            .onMove { from, to in
                guard let index = from.first else { return }
                store.moveSplit(splits[index], to: to > index ? to - 1 : to)
            }
            .onDelete { rows in rows.forEach { store.removeSplit(splits[$0]) } }
            NavigationLink {
                SplitPage(split: store.newSplit())
            } label: {
                SettingsActionLabel(title: "Add Split", symbol: .plus)
            }
        }
        RulesSection()
        let tabs = store.savedTabs
        SettingsListSection("Tabs", footer: "The mailboxes along the bottom. Hold a tab and drag it to move it; swipe it to remove it.") {
            ForEach(tabs) { tab in
                Label {
                    Text(store.tabName(tab)).foregroundStyle(Tokens.foreground.color)
                } icon: {
                    SettingsTile(symbol: Symbol.named(tab.symbol))
                }
            }
            .onMove { from, to in
                guard let index = from.first else { return }
                store.moveTab(tabs[index], to: to > index ? to - 1 : to)
            }
            .onDelete { rows in rows.forEach { store.toggleTab(tabs[$0].id) } }
            NavigationLink {
                AddTabPage()
            } label: {
                SettingsActionLabel(title: "Add Tab", symbol: .plus)
            }
        }
        .id("tabs")
        SwipeSection()
    }
}

/// The rules, each pushing its editor, and the way to add one.
private struct RulesSection: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let rules = store.customRules
        SettingsListSection(
            "Rules", footer: "Mail that matches is labelled, kept out of the inbox or marked read as it arrives. Hold a rule and drag it to move it; swipe it to remove it."
        ) {
            ForEach(rules) { rule in
                NavigationLink {
                    RulePage(draft: store.draft(of: rule))
                } label: {
                    Label {
                        Text(store.summary(of: rule)).foregroundStyle(Tokens.foreground.color).lineLimit(2)
                    } icon: {
                        SettingsTile(symbol: .zap)
                    }
                }
            }
            .onMove { from, to in
                guard let index = from.first else { return }
                store.moveRule(rules[index], to: to > index ? to - 1 : to)
            }
            .onDelete { rows in rows.forEach { store.removeRule(rules[$0]) } }
            NavigationLink {
                RulePage(draft: store.newRule())
            } label: {
                SettingsActionLabel(title: "Add Rule", symbol: .plus)
            }
        }
        .id("rules")
    }
}

/// What a swipe to the left on a thread does: one to three commands, the first at the edge.
private struct SwipeSection: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let commands = store.swipeCommands
        SettingsListSection("Swipe Left", footer: footer(count: commands.count)) {
            ForEach(commands) { command in
                HStack(spacing: Space.m) {
                    SwipeTile(command: command)
                    Text(command.swipeName).foregroundStyle(Tokens.foreground.color)
                    Spacer(minLength: Space.s)
                    if command == commands.first {
                        Text(commands.count == 1 ? "Short swipe" : "Long swipe").textStyle(.caption, color: Tokens.mutedForeground.color)
                    }
                }
                .deleteDisabled(commands.count == 1)
            }
            .onMove { from, to in
                guard let index = from.first else { return }
                store.moveSwipeCommand(commands[index], to: to > index ? to - 1 : to)
            }
            .onDelete { rows in
                store.setSwipeCommands(commands.enumerated().filter { !rows.contains($0.offset) }.map(\.element))
            }
            if commands.count < SwipePreference.most {
                NavigationLink {
                    AddSwipePage()
                } label: {
                    SettingsActionLabel(title: "Add Action", symbol: .plus)
                }
            }
        }
        .id("swipe")
    }

    private func footer(count: Int) -> String {
        guard count > 1 else { return "Swipe a thread to the left to show it; swipe a little further to do it. Add another to choose between them." }
        return "Swipe a thread to the left to choose one, the first at the edge; swipe it all the way to do the first. Hold one and drag it to move it; swipe it to remove it."
    }
}

/// The commands a swipe doesn't have yet; the one picked joins them and the page goes back.
private struct AddSwipePage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        SettingsList {
            Section {
                ForEach(store.swipeChoicesLeft) { command in
                    Button {
                        store.setSwipeCommands(store.swipeCommands + [command])
                        dismiss()
                    } label: {
                        HStack(spacing: Space.m) {
                            SwipeTile(command: command)
                            Text(command.swipeName).foregroundStyle(Tokens.foreground.color)
                        }
                    }
                }
            }
        }
        .navigationTitle("Add Action")
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// A swipe command as the swipe shows it: its icon in a wash of its colour.
private struct SwipeTile: View {
    let command: ThreadCommand

    var body: some View {
        let tint = command.swipeTint.fill
        Image(command.symbol, size: 15)
            .foregroundStyle(tint.color)
            .frame(width: SettingsTile.size, height: SettingsTile.size)
            .background(tint.opacity(Opacity.colorTintStronger).color, in: Circle())
    }
}

/// The mailboxes that are not tabs yet; the one picked joins them and the page goes back.
private struct AddTabPage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let left = store.mailboxesForTabs
        SettingsList {
            if !left.unified.isEmpty {
                Section {
                    ForEach(left.unified) { mailbox in row(mailbox) }
                }
            }
            ForEach(left.accounts, id: \.account.id) { account, mailboxes in
                SettingsListSection(account.title) {
                    ForEach(mailboxes) { mailbox in row(mailbox) }
                }
            }
        }
        .navigationTitle("Add Tab")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func row(_ mailbox: Mailbox) -> some View {
        Button {
            store.toggleTab(mailbox.id)
            dismiss()
        } label: {
            Label {
                Text(mailbox.name).foregroundStyle(Tokens.foreground.color)
            } icon: {
                SettingsTile(symbol: Symbol.named(mailbox.symbol))
            }
        }
    }
}

/// A split being written: its name, its senders, its label.
private struct SplitPage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State var split: CustomSplit
    @State private var senders = ""

    var body: some View {
        let isNew = !store.customSplits.contains { $0.id == split.id }
        let labels = store.splitLabels
        SettingsList {
            SettingsListSection("Name") {
                TextField("Team", text: $split.name)
            }
            SettingsListSection("From", footer: "Addresses, or @ and a domain for everyone there.") {
                TextField("ann@example.com, @example.com", text: $senders, axis: .vertical)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.emailAddress)
            }
            if !labels.isEmpty {
                SettingsListSection(footer: "Mail with the label goes to the split too.") {
                    Picker("Label", selection: $split.label) {
                        Text("None").tag(String?.none)
                        ForEach(labels, id: \.id) { label in Text(label.name).tag(Optional(label.id)) }
                    }
                }
            }
            if !isNew {
                Section {
                    Button("Remove Split", role: .destructive) {
                        store.removeSplit(split)
                        dismiss()
                    }
                    .foregroundStyle(Tokens.destructive.color)
                }
            }
        }
        .navigationTitle(isNew ? "New Split" : "Edit Split")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    split.name = split.name.trimmingCharacters(in: .whitespaces)
                    split.from = CustomSplit.senders(senders)
                    store.saveSplit(split)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(split.name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .onAppear { senders = split.from.joined(separator: ", ") }
    }
}

/// A rule being written: what mail it looks for, and what it does to it.
private struct RulePage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State var draft: RuleDraft

    var body: some View {
        let labels = store.splitLabels
        let accounts = store.accounts
        SettingsList {
            SettingsListSection("From", footer: "Addresses, or @ and a domain for everyone there.") {
                addressField("ann@example.com, @example.com", text: $draft.from)
            }
            SettingsListSection("To", footer: "Looks at To and Cc.") {
                addressField("team@example.com", text: $draft.to)
            }
            SettingsListSection("Subject Contains", footer: "Mail matches when it meets every field filled in.") {
                TextField("[project], invoice", text: $draft.subject, axis: .vertical)
                    .autocorrectionDisabled()
            }
            SettingsListSection("Then") {
                if !accounts.isEmpty {
                    Picker("Label", selection: $draft.label) {
                        Text("None").tag(RuleDraft.LabelChoice?.none)
                        ForEach(labels, id: \.id) { label in Text(label.name).tag(Optional(RuleDraft.LabelChoice.existing(label.id))) }
                        Text("New Label").tag(Optional(RuleDraft.LabelChoice.new))
                    }
                }
                if draft.label == .new {
                    TextField("Label name", text: $draft.newLabelName)
                    if accounts.count > 1 {
                        Picker("Account", selection: $draft.newLabelAccount) {
                            ForEach(accounts) { account in Text(account.title).tag(Optional(account.id)) }
                        }
                    }
                }
                Toggle("Skip the Inbox", isOn: $draft.rule.skipInbox)
                Toggle("Mark as Read", isOn: $draft.rule.markRead)
            }
            if draft.isNew {
                SettingsListSection(footer: "What matches now is done at once, and can be undone.") {
                    Toggle("Also Apply to Mail Already Here", isOn: $draft.applying)
                }
            } else {
                Section {
                    Button("Remove Rule", role: .destructive) {
                        store.removeRule(draft.rule)
                        dismiss()
                    }
                    .foregroundStyle(Tokens.destructive.color)
                }
            }
        }
        .navigationTitle(draft.isNew ? "New Rule" : "Edit Rule")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    store.saveRule(draft)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(!draft.canSave)
            }
        }
    }

    private func addressField(_ placeholder: String, text: Binding<String>) -> some View {
        TextField(placeholder, text: text, axis: .vertical)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.emailAddress)
    }
}

// MARK: Reading

private struct ReadingPage: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        SettingsListSection(footer: "In dark mode, newsletters and other designed mail are drawn in dark colours. Each message can show the original.") {
            Toggle("Dark Mode for Mail", isOn: store.flag("dark_mail", default: true))
        }
        .id("mail")
        SettingsListSection(footer: "Off, pictures hosted by the sender wait until you ask for them, so nobody learns when you read their mail.") {
            Toggle("Load Remote Images", isOn: store.flag("remote_images", default: true))
        }
        SettingsListSection(footer: "Their new mail goes to the trash. Block someone from a message of theirs.") {
            NavigationLink {
                BlockedPage()
            } label: {
                LabeledContent("Blocked Senders", value: store.blockedSenders.isEmpty ? "None" : "\(store.blockedSenders.count)")
            }
        }
        .id("blocked")
    }
}

private struct BlockedPage: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        let blocked = store.blockedSenders
        SettingsList {
            SettingsListSection(footer: blocked.isEmpty ? nil : "Swipe a sender to unblock them.") {
                if blocked.isEmpty {
                    Text("No one is blocked.").foregroundStyle(Tokens.mutedForeground.color)
                }
                ForEach(blocked, id: \.self) { address in
                    Text(address)
                        .swipeActions {
                            Button("Unblock", role: .destructive) { store.unblock(address) }
                        }
                }
            }
        }
        .navigationTitle("Blocked Senders")
        .navigationBarTitleDisplayMode(.inline)
    }
}

// MARK: Connected apps

/// The MCP server to add Wonnet to an application with, to copy, and the applications let in, each
/// swiped to revoke.
private struct ConnectedAppsPage: View {
    @Environment(MailStore.self) private var store
    @State private var model = ConnectedAppsModel()
    @State private var revoking: ConnectedApp?

    var body: some View {
        SettingsListSection("MCP Server", footer: "Use this URL with MCP clients such as Claude or ChatGPT. You can pick what each one can access.") {
            if let address = model.address {
                AddressRow(address: address)
            } else {
                Spinner()
            }
        }
        .id("connect")
        SettingsListSection(
            "Connected apps",
            footer: "They reach your mail as you chose. Tap one to change what it may do; swipe it to revoke it: its access ends at once."
        ) {
            if let apps = model.apps {
                if apps.isEmpty {
                    Text("No applications are connected yet.").foregroundStyle(Tokens.mutedForeground.color)
                }
                ForEach(apps) { app in
                    Button { store.change(app) } label: { appRow(app) }
                        .buttonStyle(.plain)
                        .swipeActions {
                            Button("Revoke", role: .destructive) { revoking = app }
                        }
                }
            } else if model.error == nil {
                HStack(spacing: Space.s) {
                    Spinner()
                    Text("Loading").foregroundStyle(Tokens.mutedForeground.color)
                }
            }
            if let error = model.error {
                Notice(text: error, tone: .error, symbol: .circleAlert, action: "Try again") {
                    Task { await model.load(store) }
                }
            }
        }
        .id("connected")
        .task(id: store.connectedAppsRevision) { await model.load(store) }
        .confirmationDialog(
            "Revoke \(revoking?.name ?? "")?", isPresented: Binding(get: { revoking != nil }, set: { if !$0 { revoking = nil } }),
            titleVisibility: .visible, presenting: revoking
        ) { app in
            Button("Revoke", role: .destructive) { model.revoke(app, store) }
        } message: { _ in
            Text("It can no longer reach your mail. You can connect it again.")
        }
    }

    private func appRow(_ app: ConnectedApp) -> some View {
        HStack(spacing: Space.m) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: Space.s) {
                    Text(app.name).foregroundStyle(Tokens.foreground.color).lineLimit(1)
                    if app.verified {
                        Chip(title: "Verified", symbol: .shieldCheck, variant: .success)
                    }
                }
                Text(app.access).textStyle(.caption, color: Tokens.mutedForeground.color).fixedSize(horizontal: false, vertical: true)
                Text(app.used).textStyle(.caption, color: Tokens.mutedStrongerForeground.color)
            }
            Spacer(minLength: Space.s)
            if model.revoking.contains(app.id) {
                Spinner()
            } else {
                Image(.chevronRight, size: 13).foregroundStyle(Tokens.mutedStrongerForeground.color)
            }
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
    }
}

/// The MCP server's address as a row of the list, with the copy button at its end, which shows a
/// check for a moment once it has copied.
private struct AddressRow: View {
    let address: String
    @State private var copied = false

    var body: some View {
        HStack(spacing: Space.m) {
            Text(address).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
            Spacer(minLength: Space.s)
            Button {
                Platform.copy(address)
                copied = true
                Task {
                    try? await Task.sleep(for: .seconds(2))
                    copied = false
                }
            } label: {
                Image(copied ? .check : .copy, size: 17)
            }
            .buttonStyle(.borderless)
            .foregroundStyle(Tokens.primary.color)
            .accessibilityLabel(copied ? "Copied" : "Copy the address")
        }
    }
}

// MARK: Writing

private struct WritingPage: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        @Bindable var store = store
        let snippets = store.snippets
        SettingsListSection(footer: "How long a message waits after Send, to be taken back.") {
            Picker("Undo Send", selection: $store.undoDelay) {
                ForEach(MailStore.undoDelays, id: \.self) { Text(MailStore.undoDelayName($0)).tag($0) }
            }
        }
        .id("sending")
        SettingsListSection("Snippets", footer: "Type ; and a name while writing to put one in. {first_name} becomes the first recipient's first name.") {
            ForEach(snippets) { snippet in
                NavigationLink {
                    SnippetPage(snippet: snippet)
                } label: {
                    SettingsItem(title: snippet.name, detail: snippet.text.replacingOccurrences(of: "\n", with: " ")) { EmptyView() }
                }
            }
            .onDelete { rows in rows.forEach { store.setPreference(snippets[$0].key, nil) } }
            NavigationLink {
                SnippetPage(snippet: Snippet(id: UUID().uuidString.lowercased(), name: "", text: ""))
            } label: {
                SettingsActionLabel(title: "Add Snippet", symbol: .plus)
            }
        }
        .id("snippets")
    }
}

private struct SnippetPage: View {
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State var snippet: Snippet

    var body: some View {
        let isNew = !store.snippets.contains { $0.id == snippet.id }
        SettingsList {
            SettingsListSection("Name") {
                TextField("thanks", text: $snippet.name)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            SettingsListSection("Text", footer: "{first_name} becomes the first recipient's first name.") {
                TextField("Thanks {first_name}!", text: $snippet.text, axis: .vertical)
                    .lineLimit(6...16)
            }
            if !isNew {
                Section {
                    Button("Remove Snippet", role: .destructive) {
                        store.setPreference(snippet.key, nil)
                        dismiss()
                    }
                    .foregroundStyle(Tokens.destructive.color)
                }
            }
        }
        .navigationTitle(isNew ? "New Snippet" : "Edit Snippet")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    store.saveSnippet(snippet)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(snippet.name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }
}

// MARK: Parts

/// The settings' list: the system's inset groups, drawn on the theme's colours.
struct SettingsList<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        List {
            content
                .listRowBackground(Tokens.card.color)
                .listRowSeparatorTint(Tokens.borderCard.color)
        }
        .listStyle(.insetGrouped)
        // Room after the last group, so it doesn't end against a bar under the list.
        .contentMargins(.bottom, Space.xxl + Space.xl, for: .scrollContent)
        .scrollContentBackground(.hidden)
        .background(Tokens.background.color.ignoresSafeArea())
        .font(TextStyle.body.font)
        .foregroundStyle(Tokens.foreground.color)
    }
}

/// A group of the list: its name over it and the muted line under it saying what it does, as
/// the Mac's. Both are words, so every group looks the same.
struct SettingsListSection<Content: View>: View {
    let title: String?
    let footer: String?
    @ViewBuilder let content: Content

    init(_ title: String? = nil, footer: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.footer = footer
        self.content = content()
    }

    var body: some View {
        Section {
            content
        } header: {
            if let title { Text(title).textStyle(.subheading).textCase(nil) }
        } footer: {
            if let footer { Text(footer).textStyle(.caption, color: Tokens.mutedForeground.color) }
        }
    }
}

/// A row's picture: its symbol, in a square as wide as a dot's beside it.
private struct SettingsTile: View {
    static let size: CGFloat = 28
    let symbol: Symbol

    var body: some View {
        Image(symbol, size: 17)
            .foregroundStyle(Tokens.foreground.color)
            .frame(width: Self.size, height: Self.size)
    }
}

/// A row that adds something: its symbol and its words, in the tint.
private struct SettingsActionLabel: View {
    let title: String
    let symbol: Symbol

    var body: some View {
        Label {
            Text(title)
        } icon: {
            Image(symbol, size: 17).frame(width: SettingsTile.size, height: SettingsTile.size)
        }
        .foregroundStyle(Tokens.primary.color)
    }
}

/// A row that names something, with a line about it under and its picture before it.
private struct SettingsItem<Picture: View>: View {
    let title: String
    let detail: String
    @ViewBuilder let picture: Picture

    var body: some View {
        HStack(spacing: Space.m) {
            picture
            VStack(alignment: .leading, spacing: 2) {
                Text(title).foregroundStyle(Tokens.foreground.color).lineLimit(1)
                Text(detail).textStyle(.caption, color: Tokens.mutedForeground.color).lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}
#endif
