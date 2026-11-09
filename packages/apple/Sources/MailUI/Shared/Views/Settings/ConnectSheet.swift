import SwiftUI

/// An application asking to reach the mail, opened by the `wonnet://connect` link of the page it
/// sent the browser to: who is asking, which accounts it reaches, its role and its privileges.
/// Deny and Approve sit side by side under the choices; closing the sheet denies too, so the
/// browser doesn't wait on an answer that won't come.
struct ConnectSheet: View {
    let url: String
    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var phase: Phase = .loading
    @State private var everyAccount = true
    @State private var accounts: Set<String> = []
    @State private var role = "reader"
    @State private var privileges: Set<String> = []
    @State private var approving = false
    @State private var denying = false
    @State private var failure: String?
    @State private var answered = false

    private enum Phase {
        case loading
        case failed(String)
        case asking(ConnectRequest)
        case approved(String)
    }

    var body: some View {
        Sheet(title: title, size: .large, background: Tokens.background) {
            switch phase {
            case .loading:
                Spinner(size: 20).frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                ended(symbol: .circleAlert, tint: Tokens.destructive, title: "This request can't go on", message: message)
            case .asking(let request):
                asking(request)
            case .approved(let client):
                ended(
                    symbol: .circleCheck, tint: Tokens.success, title: "Approved",
                    message: "Your browser takes you back to \(client). You can revoke it in Settings → Connected apps."
                )
            }
        }
        .task { await load() }
        .onDisappear {
            guard !answered, case .asking(let request) = phase else { return }
            store.denyQuietly(request)
        }
    }

    private var title: String {
        switch phase {
        case .asking(let request): "Connect \(request.client)"
        case .approved(let client): "Connect \(client)"
        default: "Connect an application"
        }
    }

    private func load() async {
        do {
            phase = .asking(try await store.connectRequest(url))
        } catch {
            phase = .failed(error.localizedDescription)
        }
    }

    // MARK: Asking

    @ViewBuilder private func asking(_ request: ConnectRequest) -> some View {
        #if os(iOS)
        iosForm(request)
        #else
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Space.xl) {
                    Asker(request: request)
                    choices(request)
                    if let failure {
                        Notice(text: failure, tone: .error, symbol: .circleAlert)
                    }
                }
                .padding(Space.xl)
                .frame(maxWidth: Theme.settingsPageWidth)
                .frame(maxWidth: .infinity)
            }
            HStack(spacing: 0) {
                Spacer(minLength: 0)
                ActionButton(title: "Deny", variant: .ghost, pending: denying) { deny(request) }
                    .disabled(approving)
                ActionButton(title: "Approve", symbol: .check, variant: .primary, pending: approving) { approve(request) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!(everyAccount || !accounts.isEmpty) || denying)
            }
            .padding(.horizontal, Space.xl - Theme.buttonGap / 2)
            .padding(.vertical, Space.m)
            .rule(.top)
        }
        #endif
    }

    #if os(iOS)
    /// On iOS the system's own: a list of inset groups, a checkmark on the choice in force,
    /// switches for the privileges, and the system's buttons under it.
    private func iosForm(_ request: ConnectRequest) -> some View {
        SettingsList {
            Section {
                Asker(request: request, card: false)
            }
            SettingsListSection("Accounts", footer: "Which mail it can reach.") {
                CheckCell(title: "Every account", detail: "Including the ones you add later.", symbol: .inbox, checked: everyAccount) {
                    everyAccount = true
                }
                CheckCell(title: "Only some accounts", detail: "Only the ones you pick.", symbol: .listFilter, checked: !everyAccount) {
                    everyAccount = false
                }
            }
            if !everyAccount {
                SettingsListSection(footer: accounts.isEmpty ? "Pick at least one account." : nil) {
                    ForEach(request.accounts) { account in
                        CheckCell(
                            title: account.title, detail: account.detail, dot: Theme.accountColor(account.color ?? "").color,
                            checked: accounts.contains(account.id)
                        ) { toggle(account.id, in: &accounts) }
                    }
                }
            }
            SettingsListSection("Role", footer: "The most it can do with that mail.") {
                ForEach(request.roles) { choice in
                    CheckCell(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), checked: role == choice.id) {
                        role = choice.id
                    }
                }
            }
            SettingsListSection("Privileges", footer: "What the role doesn't cover.") {
                ForEach(request.privileges) { choice in
                    Toggle(isOn: Binding(get: { privileges.contains(choice.id) }, set: { _ in toggle(choice.id, in: &privileges) })) {
                        CellLabel(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named))
                    }
                }
            }
            if let failure {
                Section {
                    Label { Text(failure) } icon: { Image(.circleAlert, size: 17) }
                        .foregroundStyle(Tokens.destructive.color)
                }
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .bottomBar) {
                Spacer()
                Button("Deny") { deny(request) }
                    .tint(Tokens.foreground.color)
                    .disabled(approving || denying)
                Button { approve(request) } label: {
                    if approving { ProgressView() } else { Text("Approve") }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!(everyAccount || !accounts.isEmpty) || denying || approving)
            }
        }
    }
    #endif

    @ViewBuilder private func choices(_ request: ConnectRequest) -> some View {
        FormSection(title: "Accounts", detail: "Which mail it can reach.") {
            SettingsCard {
                MarkRow(title: "Every account", detail: "Including the ones you add later.", symbol: .inbox, checked: everyAccount) {
                    everyAccount = true
                }
                SettingsDivider()
                MarkRow(title: "Only some accounts", detail: "Only the ones you pick.", symbol: .listFilter, checked: !everyAccount) {
                    everyAccount = false
                }
                if !everyAccount {
                    ForEach(request.accounts) { account in
                        SettingsDivider()
                        MarkRow(
                            title: account.title, detail: account.detail, dot: Theme.accountColor(account.color ?? "").color, kind: .any,
                            indent: Space.xl, checked: accounts.contains(account.id)
                        ) { toggle(account.id, in: &accounts) }
                    }
                }
            }
        }
        FormSection(title: "Role", detail: "The most it can do with that mail.") {
            SettingsCard {
                ForEach(request.roles) { choice in
                    if choice != request.roles.first { SettingsDivider() }
                    MarkRow(title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), checked: role == choice.id) {
                        role = choice.id
                    }
                }
            }
        }
        FormSection(title: "Privileges", detail: "What the role doesn't cover.") {
            SettingsCard {
                ForEach(request.privileges) { choice in
                    if choice != request.privileges.first { SettingsDivider() }
                    MarkRow(
                        title: choice.title, detail: choice.detail, symbol: choice.symbol.map(Symbol.named), kind: .any,
                        checked: privileges.contains(choice.id)
                    ) { toggle(choice.id, in: &privileges) }
                }
            }
        }
    }

    private func toggle(_ id: String, in set: inout Set<String>) {
        if set.contains(id) { set.remove(id) } else { set.insert(id) }
    }

    private func approve(_ request: ConnectRequest) {
        let access: [String: Any] = [
            "role": role,
            "all_accounts": everyAccount,
            "accounts": everyAccount ? [] : request.accounts.map(\.id).filter(accounts.contains),
            "privileges": request.privileges.map(\.id).filter(privileges.contains),
        ]
        approving = true
        failure = nil
        Task {
            defer { approving = false }
            do {
                try await store.approve(request, access: access)
                answered = true
                withAnimation { phase = .approved(request.client) }
            } catch {
                failure = error.localizedDescription
            }
        }
    }

    private func deny(_ request: ConnectRequest) {
        denying = true
        failure = nil
        Task {
            defer { denying = false }
            do {
                try await store.deny(request)
                answered = true
                store.show("Denied \(request.client).")
                dismiss()
            } catch {
                failure = error.localizedDescription
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
            Button("Done") { dismiss() }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.top, Space.s)
            #else
            ActionButton(title: "Done", variant: .primary) { dismiss() }
                .keyboardShortcut(.defaultAction)
                .padding(.top, Space.s)
            #endif
        }
        .frame(maxWidth: Theme.formWidth)
        .padding(Space.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                fact(.arrowRight, request.backTo)
                if request.onDevice {
                    fact(.monitor, "Sends you back to an application on this device", warning: true)
                }
                fact(.circleAlert, "Only approve if you started this yourself", warning: true)
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
/// A choice as a row of iOS's list: its symbol or an account's dot, its name and a line, and the
/// system's checkmark at its end when it is in force. The whole row is the button.
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
                if checked {
                    Image(.check, size: 17).foregroundStyle(Tokens.primary.color)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(checked ? .isSelected : [])
    }
}

/// A row's words with its picture before them: a symbol in the row's tile, or an account's dot.
private struct CellLabel: View {
    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?

    var body: some View {
        HStack(spacing: Space.m) {
            Group {
                if let dot {
                    Circle().fill(dot).frame(width: 10, height: 10)
                } else if let symbol {
                    Image(symbol, size: 17).foregroundStyle(Tokens.foreground.color)
                }
            }
            .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).foregroundStyle(Tokens.foreground.color)
                if let detail {
                    Text(detail).textStyle(.caption, color: Tokens.mutedForeground.color).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(.vertical, 2)
    }
}
#endif
