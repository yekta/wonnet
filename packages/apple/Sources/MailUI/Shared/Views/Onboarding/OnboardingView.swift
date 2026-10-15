import SwiftUI

/// The first screen: add a mail account, which also signs in.
struct OnboardingView: View {
    var body: some View {
        #if os(iOS)
        NavigationStack {
            AddAccountFlow(welcome: true).toolbar(.hidden, for: .navigationBar)
        }
        .tint(Tokens.foreground.color)
        #else
        AddAccountFlow(welcome: true)
        #endif
    }
}

/// The ways to add an account: Google, or a JMAP server on a page of its own. On the first
/// screen (`welcome`) the app's name stands over them and the server this app talks to can be
/// changed under them, on a page of its own too. iOS pushes the pages; the Mac shows one in
/// place, with a way back.
struct AddAccountFlow: View {
    var welcome = false
    @Environment(MailStore.self) private var store
    @State private var page: Page?

    enum Page: Hashable {
        case jmap, server
    }

    var body: some View {
        #if os(iOS)
        start.navigationDestination(item: $page) { page in
            Column(centered: welcome) { form(page) }.tint(Tokens.primary.color)
        }
        #else
        ZStack {
            if let page {
                Column(centered: welcome) {
                    VStack(alignment: .leading, spacing: Space.m) {
                        ActionButton(title: "Back", symbol: .arrowLeft, variant: .ghost, size: .small) { show(nil) }
                            .padding(.leading, -(ControlSize.small.height / 2 + Theme.buttonGap / 2))
                        form(page)
                    }
                }
                .transition(.offset(x: Space.xxl).combined(with: .opacity))
            } else {
                start.transition(.offset(x: -Space.xxl).combined(with: .opacity))
            }
        }
        #endif
    }

    @ViewBuilder private func form(_ page: Page) -> some View {
        switch page {
        case .jmap: JmapForm()
        case .server: ServerForm(done: { show(nil) })
        }
    }

    @ViewBuilder private var start: some View {
        if welcome {
            Column(centered: true) {
                VStack(spacing: Space.xxl + Space.s) {
                    VStack(spacing: Space.l) {
                        AppMark()
                        VStack(spacing: Space.xxs) {
                            Text("Wonnet").textStyle(.display)
                            Text("Email that feels instant.").textStyle(.body, color: Tokens.mutedForeground.color)
                        }
                    }
                    choices
                }
            }
            .safeAreaInset(edge: .bottom) {
                ActionButton(title: "Use your own server", symbol: .server, variant: .ghost, size: .small) { show(.server) }
                    .padding(Space.l)
            }
        } else {
            Column { choices }
        }
    }

    private var choices: some View {
        VStack(spacing: Space.s) {
            OptionButton(title: "Continue with Google", detail: "Gmail and Google Workspace", action: store.signInWithGoogle) {
                GoogleMark()
            }
            OptionButton(title: "Continue with JMAP", detail: "Fastmail, Stalwart and others", action: { show(.jmap) }) {
                Image(.server, size: 20).foregroundStyle(Tokens.foreground.color)
            }
        }
    }

    private func show(_ page: Page?) {
        #if os(iOS)
        self.page = page
        #else
        withAnimation(.easeOut(duration: 0.18)) { self.page = page }
        #endif
    }
}

/// A form's column: as wide as a form, in the middle of the page, scrolled when the keyboard
/// leaves it no room. `centered` on a whole screen, a little above the middle, where the eye
/// rests; at the top of a sheet or a settings page.
private struct Column<Content: View>: View {
    var centered = false
    @ViewBuilder let content: Content

    /// How much of the screen's height the centred content sits above its middle by.
    private let lift: CGFloat = 0.08

    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack(spacing: 0) {
                    if centered { Spacer(minLength: 0) }
                    content
                        .frame(maxWidth: Theme.formWidth)
                        .padding(.horizontal, Space.xl)
                        .padding(.vertical, centered ? Space.xxl : Space.xl)
                    if centered {
                        Spacer(minLength: 0)
                        Color.clear.frame(height: geometry.size.height * lift)
                    }
                }
                .frame(maxWidth: .infinity, minHeight: centered ? geometry.size.height : 0)
            }
        }
        .background(Tokens.background.color.ignoresSafeArea())
    }
}

/// A JMAP server, an address and an app password.
private struct JmapForm: View {
    @Environment(MailStore.self) private var store
    @State private var url = ""
    @State private var email = ""
    @State private var password = ""
    @State private var adding = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xl) {
            FormHeading(title: "JMAP server", detail: "Any JMAP server, with an app password.")
            VStack(spacing: Space.m) {
                InputField(label: "Server", text: $url, placeholder: "https://api.fastmail.com", autofocus: true)
                InputField(label: "Email", text: $email, placeholder: "you@example.com")
                InputField(label: "Password", text: $password, placeholder: "App password", secure: true, commit: add)
            }
            if let error {
                Notice(text: error, tone: .error)
            }
            ActionButton(title: "Add account", variant: .primary, pending: adding, wide: true, action: add)
                .disabled(url.isEmpty || email.isEmpty || password.isEmpty)
        }
    }

    private func add() {
        guard !adding, !url.isEmpty, !email.isEmpty, !password.isEmpty else { return }
        adding = true
        error = nil
        Task {
            do {
                try await store.addJmap(url: url, username: email, password: password)
                password = ""
            } catch {
                self.error = error.localizedDescription
            }
            adding = false
        }
    }
}

/// The server this app talks to: ours, or one the user runs.
private struct ServerForm: View {
    @Environment(MailStore.self) private var store
    let done: () -> Void
    @State private var server = ""
    @State private var saving = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xl) {
            FormHeading(title: "Your own server", detail: "The app starts on Wonnet's server. Running your own, point this \(Platform.device) at it.")
            InputField(label: "Server", text: $server, placeholder: "https://mail.example.com", autofocus: true, commit: save)
            if let error {
                Notice(text: error, tone: .error)
            }
            ActionButton(title: "Use this server", variant: .primary, pending: saving, wide: true, action: save)
                .disabled(server.isEmpty)
        }
        .onAppear { server = store.server }
    }

    private func save() {
        guard !saving, !server.isEmpty else { return }
        saving = true
        error = nil
        Task {
            do {
                try await store.setServer(server)
                done()
            } catch {
                self.error = error.localizedDescription
            }
            saving = false
        }
    }
}

/// A page's name and the line under it, over its form.
private struct FormHeading: View {
    let title: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            Text(title).textStyle(.title)
            Text(detail).textStyle(.body, color: Tokens.mutedForeground.color).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
