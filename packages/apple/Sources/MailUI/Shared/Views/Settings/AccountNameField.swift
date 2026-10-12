import SwiftUI

/// What the user calls an account, saved on Return and when the field is left: an `InputField` at
/// the end of the Mac's row, the system's field in iOS's list.
struct AccountNameField: View {
    @Environment(MailStore.self) private var store
    let account: AccountView
    @State private var text: String
    @FocusState private var focused: Bool

    init(account: AccountView) {
        self.account = account
        _text = State(initialValue: account.name)
    }

    var body: some View {
        field
            .onChange(of: account.name) { _, name in text = name }
            .onDisappear(perform: save)
    }

    @ViewBuilder private var field: some View {
        let limited = Binding(get: { text }, set: { text = String($0.prefix(AccountView.nameMax)) })
        #if os(macOS)
        InputField(text: limited, placeholder: Self.placeholder, commit: save)
            .frame(width: Theme.settingsFieldWidth)
        #else
        TextField("Name", text: limited, prompt: Text(Self.placeholder))
            .multilineTextAlignment(.trailing)
            .submitLabel(.done)
            .focused($focused)
            .onSubmit(save)
            .onChange(of: focused) { _, now in
                guard !now else { return }
                save()
            }
        #endif
    }

    private static let placeholder = "Work, Personal"

    private func save() {
        store.setAccountName(account, text)
    }
}
