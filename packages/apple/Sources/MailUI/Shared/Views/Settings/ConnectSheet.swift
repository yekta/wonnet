import SwiftUI

/// An application asking to reach the mail, opened by the `wonnet://connect` link of the page it
/// sent the browser to, or a connected app whose access is changed: who it is, which accounts and
/// which of their mail it reaches (the filter), what it reads of each message and what it can do,
/// and the sentence that says it all. On the Mac it is a page over the whole window, as the
/// settings are: the choices in a column of their own, and beside them who asks, the sentence and
/// the buttons that answer, so neither covers the other. On iOS it is a sheet. Leaving a request
/// unanswered denies it, so the browser doesn't wait on an answer that won't come.
struct ConnectSheet: View {
    let connecting: Connecting
    @Environment(MailStore.self) private var store
    @State private var phase: Phase = .loading
    @State private var approving = false
    @State private var denying = false
    @State private var answered = false

    private enum Phase {
        case loading
        case failed(String)
        case asking(AccessDraft)
        case approved(String)
    }

    var body: some View {
        shell
            .task { await load() }
            .onDisappear {
                guard !answered, case .asking(let draft) = phase, !draft.editing else { return }
                store.denyQuietly(draft.request)
            }
    }

    #if os(macOS)
    /// The page: its title across the top, Back beside the window's buttons, Esc too.
    private var shell: some View {
        VStack(spacing: 0) {
            HeaderBar(title: title) { EmptyView() } trailing: { EmptyView() }
            Rule()
            content
        }
        .background(Tokens.background.color)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                IconButton(symbol: .arrowLeft, help: "Back (Esc)", action: close)
                    .keyboardShortcut(.cancelAction)
            }
            .withoutSystemGlass()
        }
    }
    #else
    private var shell: some View {
        Sheet(title: title, size: .large, background: Tokens.background) { content }
            // A swipe would answer the request, or drop the changes, without a word: Deny, Cancel or
            // the X say it.
            .interactiveDismissDisabled()
    }
    #endif

    @ViewBuilder private var content: some View {
        switch phase {
        case .loading:
            Spinner(size: 20).frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            ended(symbol: .circleAlert, tint: Tokens.destructive, title: "This request can't go on", message: message)
        case .asking(let draft):
            asking(draft)
        case .approved(let client):
            ended(
                symbol: .circleCheck, tint: Tokens.success, title: "Approved",
                message: "Your browser takes you back to \(client). You can change or revoke it in Settings → Connected apps."
            )
        }
    }

    private var title: String {
        switch phase {
        case .asking(let draft): draft.editing ? "Change \(draft.request.client)" : "Connect \(draft.request.client)"
        case .approved(let client): "Connect \(client)"
        default: connecting.app == nil ? "Connect an application" : "Change an application"
        }
    }

    private func close() {
        store.connecting = nil
    }

    private func load() async {
        do {
            let draft = AccessDraft(request: try await store.connectRequest(connecting))
            draft.refresh(store)
            phase = .asking(draft)
        } catch {
            phase = .failed(error.localizedDescription)
        }
    }

    // MARK: Asking

    @ViewBuilder private func asking(_ draft: AccessDraft) -> some View {
        #if os(iOS)
        iosForm(draft)
        #else
        HStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Space.xxl) {
                    choices(draft)
                }
                .frame(maxWidth: Theme.settingsPageWidth, alignment: .leading)
                .padding(.horizontal, Space.xl)
                .padding(.top, Space.xl)
                .padding(.bottom, Space.xxl * 2)
                .frame(maxWidth: .infinity)
            }
            .fadesTop()
            Rule(vertical: true)
            answer(draft).frame(width: Theme.connectPanelWidth)
        }
        #endif
    }

    #if os(macOS)
    /// Beside the choices: who asks, what it all comes to, and the buttons that answer. The
    /// sentence grows as long as it needs, and scrolls here alone.
    private func answer(_ draft: AccessDraft) -> some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Space.xl) {
                    Asker(request: draft.request)
                    SentenceText(parts: draft.summary?.sentence ?? [])
                    if let failure = draft.failure {
                        Notice(text: failure, tone: .error, symbol: .circleAlert)
                    }
                }
                .padding(Space.xl)
            }
            .fadesTop()
            // Half as far apart as they are from the panel's edges; each pads half a button gap itself.
            HStack(spacing: Space.xl / 2 - Theme.buttonGap) {
                ActionButton(title: draft.editing ? "Cancel" : "Deny", variant: .outline, pending: denying, wide: true) { deny(draft) }
                    .disabled(approving)
                ActionButton(title: draft.editing ? "Save" : "Approve", symbol: .check, variant: .primary, pending: approving, wide: true) {
                    approve(draft)
                }
                .keyboardShortcut(draft.menuOpen ? nil : .defaultAction)
                .disabled(!ready(draft) || denying)
            }
            .padding(.horizontal, Space.xl - Theme.buttonGap / 2)
            .padding(.vertical, Space.xl)
            .rule(.top)
        }
    }
    #endif

    private func ready(_ draft: AccessDraft) -> Bool {
        guard let summary = draft.summary else { return false }
        return summary.problem == nil
    }

    #if os(macOS)
    @ViewBuilder private func choices(_ draft: AccessDraft) -> some View {
        let request = draft.request
        let access = draft.access
        FormSection(title: "Accounts", detail: "Whose mail it can reach.") {
            SettingsCard {
                MarkRow(title: "Every account", detail: "Including the ones you add later.", symbol: .inbox, checked: access.allAccounts) {
                    draft.change(store) { $0.allAccounts = true }
                }
                SettingsDivider()
                MarkRow(title: "Only some accounts", detail: "Only the ones you pick.", symbol: .listFilter, checked: !access.allAccounts) {
                    draft.change(store) { $0.allAccounts = false }
                }
                if !access.allAccounts {
                    ForEach(request.accounts) { account in
                        SettingsDivider()
                        MarkRow(
                            title: account.title, detail: account.detail, dot: Theme.accountColor(account.color ?? "").color, kind: .any,
                            indent: Space.xl, checked: access.accounts.contains(account.id)
                        ) { draft.toggle(account.id, in: \.accounts, store) }
                    }
                }
            }
        }
        FormSection(title: "Filter", detail: "Which of that mail it can find. What's kept out isn't there for it.") {
            FilterCard(draft: draft)
        }
        FormSection(title: "What it reads", detail: "It always sees the sender, recipients, date and labels. Pick what else.") {
            SettingsCard {
                ForEach(request.reads) { choice in
                    if choice != request.reads.first { SettingsDivider() }
                    MarkRow(
                        title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), kind: .any,
                        checked: access.reads.contains(choice.id)
                    ) { draft.toggle(choice.id, in: \.reads, store) }
                }
            }
        }
        FormSection(title: "What it can do", detail: "With the mail it reaches.") {
            SettingsCard {
                ForEach(request.roles) { choice in
                    if choice != request.roles.first { SettingsDivider() }
                    MarkRow(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), checked: access.role == choice.id) {
                        draft.change(store) { $0.role = choice.id }
                    }
                }
                ForEach(request.privileges) { choice in
                    SettingsDivider()
                    MarkRow(
                        title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), kind: .any,
                        checked: access.privileges.contains(choice.id)
                    ) { draft.toggle(choice.id, in: \.privileges, store) }
                }
            }
        }
    }
    #endif

    #if os(iOS)
    /// On iOS the system's own: a list of inset groups, a checkmark on the choice in force,
    /// switches for the rest, the filter's lines as pages, and the system's buttons under it.
    private func iosForm(_ draft: AccessDraft) -> some View {
        let request = draft.request
        let access = draft.access
        return SettingsList {
            Section {
                Asker(request: request, card: false)
            }
            SettingsListSection("Accounts", footer: "Whose mail it can reach.") {
                CheckCell(title: "Every account", detail: "Including the ones you add later.", symbol: .inbox, checked: access.allAccounts) {
                    draft.change(store) { $0.allAccounts = true }
                }
                CheckCell(title: "Only some accounts", detail: "Only the ones you pick.", symbol: .listFilter, checked: !access.allAccounts) {
                    draft.change(store) { $0.allAccounts = false }
                }
            }
            if !access.allAccounts {
                SettingsListSection(footer: draft.summary?.problem) {
                    ForEach(request.accounts) { account in
                        CheckCell(
                            title: account.title, detail: account.detail, dot: Theme.accountColor(account.color ?? "").color,
                            checked: access.accounts.contains(account.id)
                        ) { draft.toggle(account.id, in: \.accounts, store) }
                    }
                }
            }
            Section {
                NavigationLink { FilterLinePage(draft: draft, line: .only) } label: {
                    LabeledContent { Text(count(draft, .only, none: "All mail")) } label: {
                        CellLabel(title: "Only", symbol: .listFilter)
                    }
                }
                NavigationLink { FilterLinePage(draft: draft, line: .never) } label: {
                    LabeledContent { Text(count(draft, .never, none: "Nothing")) } label: {
                        CellLabel(title: "Never", symbol: .ban)
                    }
                }
            } header: {
                Text("Filter").textStyle(.subheading).textCase(nil)
            } footer: {
                ReachFooter(summary: draft.summary)
            }
            SettingsListSection("What it reads", footer: "It always sees the sender, recipients, date and labels. Pick what else.") {
                ForEach(request.reads) { choice in
                    Toggle(isOn: Binding(get: { access.reads.contains(choice.id) }, set: { _ in draft.toggle(choice.id, in: \.reads, store) })) {
                        CellLabel(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named))
                    }
                }
            }
            SettingsListSection("What it can do", footer: "With the mail it reaches.") {
                ForEach(request.roles) { choice in
                    CheckCell(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), checked: access.role == choice.id) {
                        draft.change(store) { $0.role = choice.id }
                    }
                }
                ForEach(request.privileges) { choice in
                    Toggle(
                        isOn: Binding(get: { access.privileges.contains(choice.id) }, set: { _ in draft.toggle(choice.id, in: \.privileges, store) })
                    ) {
                        CellLabel(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named))
                    }
                }
            }
            if let failure = draft.failure {
                Section {
                    Label { Text(failure) } icon: { Image(.circleAlert, size: 17) }
                        .foregroundStyle(Tokens.destructive.color)
                }
            }
        }
        // What it all comes to, over the buttons that answer: down here it grows into the bar, so
        // the rows don't move under a finger when it changes.
        .safeAreaInset(edge: .bottom, spacing: 0) {
            SentenceText(parts: draft.summary?.sentence ?? [])
                .padding(.horizontal, Space.xl)
                .padding(.vertical, Space.m)
                .background(Tokens.background.color)
                .rule(.top)
        }
        .toolbar {
            ToolbarItemGroup(placement: .bottomBar) {
                Spacer()
                Button(draft.editing ? "Cancel" : "Deny") { deny(draft) }
                    .tint(Tokens.foreground.color)
                    .disabled(approving || denying)
                Button { approve(draft) } label: {
                    if approving { ProgressView() } else { Text(draft.editing ? "Save" : "Approve") }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!ready(draft) || denying || approving)
            }
        }
    }

    private func count(_ draft: AccessDraft, _ line: FilterLine, none: String) -> String {
        let chips = draft.summary?.chips(line).count ?? 0
        return chips == 0 ? none : "\(chips)"
    }
    #endif

    private func approve(_ draft: AccessDraft) {
        approving = true
        draft.failure = nil
        Task {
            defer { approving = false }
            await draft.settled()
            guard ready(draft) else { return }
            do {
                if let app = draft.request.app {
                    try await store.saveAccess(app, access: draft.access)
                    answered = true
                    store.show("Saved.")
                    close()
                } else {
                    try await store.approve(draft.request, access: draft.access)
                    answered = true
                    withAnimation { phase = .approved(draft.request.client) }
                }
            } catch {
                draft.failure = error.localizedDescription
            }
        }
    }

    private func deny(_ draft: AccessDraft) {
        guard !draft.editing else { return close() }
        denying = true
        draft.failure = nil
        Task {
            defer { denying = false }
            do {
                try await store.deny(draft.request)
                answered = true
                store.show("Denied \(draft.request.client).")
                close()
            } catch {
                draft.failure = error.localizedDescription
            }
        }
    }

    // MARK: Ended

    private func ended(symbol: Symbol, tint: ThemeColor, title: String, message: String) -> some View {
        VStack(spacing: Space.m) {
            Image(symbol, size: 36).foregroundStyle(tint.color)
            Text(title).textStyle(.subheading)
            Text(message).textStyle(.label, color: Tokens.mutedForeground.color).multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            #if os(iOS)
            Button("Done", action: close)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.top, Space.s)
            #else
            ActionButton(title: "Done", variant: .primary, action: close)
                .keyboardShortcut(.defaultAction)
                .padding(.top, Space.s)
            #endif
        }
        .frame(maxWidth: Theme.formWidth)
        .padding(Space.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The sentence that says what an app may do, its strong runs in bold.
struct SentenceText: View {
    let parts: [SentencePart]

    var body: some View {
        parts.reduce(Text("")) { sentence, part in
            sentence + Text(part.text).fontWeight(part.strong ? .semibold : .regular)
        }
        .textStyle(.body)
        .foregroundStyle(Tokens.foreground.color)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(nil, value: parts)
    }
}

/// Who is asking: the application's name, whether it is who it says, where it comes from and
/// where it sends the browser back to.
private struct Asker: View {
    let request: ConnectRequest
    /// On a card of its own; iOS has it in a group of its list instead.
    var card = true

    var body: some View {
        if card {
            Card { content }
        } else {
            content.padding(.vertical, Space.xs)
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Space.m) {
            HStack(spacing: Space.s) {
                Image(request.verified ? .shieldCheck : .box, size: 20).foregroundStyle(Tokens.foreground.color)
                Text(request.client).textStyle(.heading).lineLimit(1)
                if request.verified {
                    Chip(title: "Verified", symbol: .shieldCheck, variant: .success)
                } else {
                    Chip(title: "Unverified", symbol: .circleAlert, variant: .warning)
                }
                Spacer(minLength: 0)
            }
            VStack(alignment: .leading, spacing: Space.xs) {
                fact(.globe, request.publisher)
                if request.app == nil {
                    fact(.arrowRight, request.backTo)
                    if request.onDevice {
                        fact(.monitor, "Sends you back to an application on this device", warning: true)
                    }
                    fact(.circleAlert, "Only approve if you started this yourself", warning: true)
                }
            }
        }
    }

    private func fact(_ symbol: Symbol, _ text: String, warning: Bool = false) -> some View {
        let color = warning ? Tokens.warning.color : Tokens.mutedForeground.color
        return HStack(alignment: .top, spacing: Space.s) {
            Image(symbol, size: 13).foregroundStyle(color).frame(height: 18 * Platform.scale)
            Text(text).textStyle(.label, color: color).fixedSize(horizontal: false, vertical: true)
        }
    }
}

#if os(iOS)
/// How much mail the access reaches, under the filter's group, and a condition that changes
/// nothing, in the warning colour.
private struct ReachFooter: View {
    let summary: AccessSummary?

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            Text(summary?.reach ?? " ").textStyle(.caption, color: Tokens.mutedForeground.color)
            if let warning = summary?.warning {
                Text(warning).textStyle(.caption, color: Tokens.warning.color)
            }
        }
    }
}

/// A line of the filter as a page of its own: its conditions, each swiped to remove it or tapped
/// to change it, and Add.
private struct FilterLinePage: View {
    let draft: AccessDraft
    let line: FilterLine
    @Environment(MailStore.self) private var store
    @State private var menu: FilterMenu?

    var body: some View {
        let chips = draft.summary?.chips(line) ?? []
        SettingsList {
            Section {
                ForEach(chips) { chip in
                    Button { menu = FilterMenu(line: line, editing: chip) } label: {
                        HStack(spacing: Space.m) {
                            CellLabel(
                                title: chip.title, detail: chip.empty ? "No mail matches it" : chip.help, symbol: chip.kind.symbol,
                                tint: chip.empty ? Tokens.warning : nil
                            )
                            Spacer(minLength: Space.s)
                            Image(.chevronRight, size: 13).foregroundStyle(Tokens.mutedStrongerForeground.color)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .swipeActions {
                        Button("Remove", role: .destructive) {
                            draft.refresh(store, edit: FilterEdit(line: line, remove: chip.filterValue))
                        }
                        .tint(Tokens.destructive.color)
                    }
                }
                Button { menu = FilterMenu(line: line, editing: nil) } label: {
                    Label {
                        Text(line == .only && chips.isEmpty ? "Narrow it" : "Add")
                    } icon: {
                        Image(.plus, size: 17).frame(width: 28)
                    }
                    .foregroundStyle(Tokens.primary.color)
                }
            } footer: {
                VStack(alignment: .leading, spacing: Space.xxs) {
                    Text(explanation).textStyle(.caption, color: Tokens.mutedForeground.color)
                    ReachFooter(summary: draft.summary)
                }
            }
        }
        .navigationTitle(line == .only ? "Only" : "Never")
        .navigationBarTitleDisplayMode(.inline)
        .sheet(item: $menu) { menu in
            FilterPicker(menu: menu, draft: draft).environment(store)
        }
    }

    private var explanation: String {
        switch line {
        case .only: "It reaches only the threads with a message that matches these: one of each kind."
        case .never: "Threads with a message that matches any of these are kept from \(draft.request.client)."
        }
    }
}

/// The filter's menu as a sheet of the whole screen: the kinds, or what in the mail matches the
/// words typed in the search box along the bottom.
private struct FilterPicker: View {
    @Bindable var menu: FilterMenu
    let draft: AccessDraft
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Sheet(title: menu.title) {
            SettingsList {
                if menu.scope != nil, let scope = menu.options?.scope, menu.editing == nil {
                    Section {
                        Button { menu.narrow(to: nil) } label: {
                            HStack(spacing: Space.m) {
                                CellLabel(title: scope, detail: "Show every kind", symbol: menu.scope?.symbol)
                                Spacer(minLength: Space.s)
                                Image(.x, size: 15).foregroundStyle(Tokens.mutedStrongerForeground.color)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
                ForEach(menu.options?.sections ?? []) { section in
                    Section {
                        ForEach(section.options) { option in
                            Button { pick(option) } label: { OptionCell(option: option) }
                                .buttonStyle(.plain)
                        }
                    } header: {
                        if let title = section.title { Text(title).textCase(nil) }
                    }
                }
                if let note = menu.options?.note {
                    Section { Text(note).foregroundStyle(Tokens.mutedForeground.color) }
                }
                if let editing = menu.editing {
                    Section {
                        Button("Remove", role: .destructive) {
                            draft.refresh(store, edit: FilterEdit(line: menu.line, remove: editing.filterValue))
                            dismiss()
                        }
                        .foregroundStyle(Tokens.destructive.color)
                    }
                }
            }
            .searchField(text: $menu.query, prompt: menu.options?.placeholder ?? "Search")
        }
        .task(id: "\(menu.scope?.rawValue ?? "")|\(menu.query)") { await menu.load(store, access: draft.access) }
    }

    private func pick(_ option: FilterOption) {
        guard let edit = menu.edit(for: option) else { return menu.narrow(to: option.kind) }
        draft.refresh(store, edit: edit)
        dismiss()
    }
}

/// An option of the filter's menu as a row of the list: its avatar or symbol, its title with its
/// detail, its count, and the check's place, kept in every row.
private struct OptionCell: View {
    let option: FilterOption

    var body: some View {
        HStack(spacing: Space.m) {
            Group {
                if let avatar = option.avatar {
                    Avatar(initials: avatar.initials, email: avatar.email, size: 28)
                } else if let symbol = option.symbol {
                    Image(Symbol.named(symbol), size: 17).foregroundStyle(Tokens.foreground.color)
                }
            }
            .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(option.title).foregroundStyle(Tokens.foreground.color).lineLimit(1)
                if let detail = option.detail, option.count != nil {
                    Text(detail).textStyle(.caption, color: Tokens.mutedForeground.color).lineLimit(1).truncationMode(.middle)
                }
            }
            Spacer(minLength: Space.s)
            if let count = option.count ?? option.detail {
                Text(count).textStyle(.caption, color: Tokens.mutedForeground.color).monospacedDigit().lineLimit(1)
            }
            Image(.check, size: 17).foregroundStyle(Tokens.primary.color).opacity(option.checked ? 1 : 0)
                .accessibilityHidden(!option.checked)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .accessibilityAddTraits(option.checked ? .isSelected : [])
    }
}

/// A choice as a row of iOS's list: its symbol or an account's dot, its name and a line, and the
/// system's checkmark at its end when it is in force, its place kept when it isn't. The whole
/// row is the button.
private struct CheckCell: View {
    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    let checked: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: Space.m) {
                CellLabel(title: title, detail: detail, symbol: symbol, dot: dot)
                Spacer(minLength: Space.s)
                Image(.check, size: 17).foregroundStyle(Tokens.primary.color).opacity(checked ? 1 : 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(checked ? .isSelected : [])
    }
}

/// A row's words with its picture before them: a symbol in the row's tile, or an account's dot.
/// `tint` colours both, for a row that warns.
private struct CellLabel: View {
    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    var tint: ThemeColor?

    var body: some View {
        HStack(spacing: Space.m) {
            Group {
                if let dot {
                    Circle().fill(dot).frame(width: 10, height: 10)
                } else if let symbol {
                    Image(symbol, size: 17).foregroundStyle((tint ?? Tokens.foreground).color)
                }
            }
            .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).foregroundStyle((tint ?? Tokens.foreground).color)
                if let detail {
                    Text(detail).textStyle(.caption, color: (tint ?? Tokens.mutedForeground).color).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(.vertical, 2)
    }
}
#endif
