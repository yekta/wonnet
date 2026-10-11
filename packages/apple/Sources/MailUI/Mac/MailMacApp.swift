#if os(macOS)
import AppKit
import SwiftUI

/// The Mac app: the list or the open thread, with the sidebar sliding over them from the left.
public struct MailMacApp: App {
    @State private var store = MailStore()
    @NSApplicationDelegateAdaptor(MacDelegate.self) private var delegate

    public init() {
        Notifier.shared.install()
    }

    private var defaultServer: String {
        Bundle.main.object(forInfoDictionaryKey: "MailServerURL") as? String ?? "http://localhost:3000"
    }

    public var body: some Scene {
        WindowGroup("Mail") {
            MacRoot()
                .environment(store)
                .preferredColorScheme(store.appearance.scheme)
                .onAppear {
                    delegate.store = store
                    store.start(defaultServer: defaultServer)
                }
                .onOpenURL { url in Task { await store.finishSignIn(url) } }
        }
        .windowToolbarStyle(.unified)
        .defaultSize(width: 1280, height: 820)
        .commands { MacCommands(store: store) }
    }
}

/// Saves where the app is, then lets it quit: SwiftUI on its own refuses to quit under a sheet.
@MainActor
final class MacDelegate: NSObject, NSApplicationDelegate {
    var store: MailStore?

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let store else { return .terminateNow }
        Task {
            await store.saveUiNow()
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}

struct MacRoot: View {
    /// Past the window's buttons, in the toolbar.
    private static let pastWindowButtons: CGFloat = 68

    @Environment(MailStore.self) private var store
    @State private var focusSearch = 0
    /// Full screen hides the window's buttons, so it has no toolbar.
    @State private var fullScreen = false

    var body: some View {
        @Bindable var store = store
        Group {
            if !store.booted {
                // The page's colour alone, until the core says where the app was left.
                Tokens.background.color
            } else if !store.signedIn {
                OnboardingView().overlay(alignment: .bottom) { ToastView() }
            } else {
                ZStack(alignment: .leading) {
                    // The mail stays under the settings, so going back finds it as it was left.
                    content
                        .opacity(store.settings == nil ? 1 : 0)
                        .allowsHitTesting(store.settings == nil)
                    if store.sidebarOpen, store.settings == nil { sidebar }
                    if let section = store.settings { SettingsRoute(section: section) }
                }
                .overlay(alignment: .bottom) { ToastView() }
                .background(KeyHandler(store: store, focusSearch: { focusSearch += 1 }, sidebarOpen: $store.sidebarOpen))
            }
        }
        .toolbar {
            // An item no one sees, for the window to have a toolbar: it puts the window's buttons
            // in the middle of a bar as tall as ours, on every page. What is beside them is ours,
            // where the system would place an item further off. Full screen has neither.
            if !fullScreen {
                ToolbarItem(placement: .navigation) {
                    Color.clear.frame(width: 1, height: 1).allowsHitTesting(false)
                }
                .withoutSystemGlass()
            }
        }
        .frame(minWidth: 860, minHeight: 520)
        .ignoresSafeArea()
        .windowToolbarUnderTopBar()
        .environment(\.pastWindowButtons, fullScreen ? 0 : Self.pastWindowButtons)
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.willEnterFullScreenNotification)) { _ in fullScreen = true }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.willExitFullScreenNotification)) { _ in fullScreen = false }
        .mailSheets(store)
    }

    /// Over the page, without moving it; a click beside it closes it.
    @ViewBuilder private var sidebar: some View {
        Tokens.overlay.opacity(Opacity.overlay).color
            .contentShape(Rectangle())
            .onTapGesture { showSidebar(false) }
            .transition(.opacity)
            .zIndex(1)
        VStack(spacing: 0) {
            // Under the window's buttons.
            Color.clear.frame(height: Theme.barHeight)
            SidebarView(
                showSettings: {
                    showSidebar(false)
                    store.openSettings()
                },
                picked: { _ in showSidebar(false) }
            )
        }
        .frame(width: Theme.sidebarWidth)
        .background(Tokens.background.color)
        .rule(.trailing)
        .transition(.move(edge: .leading))
        // Above the page while it leaves too, or the ZStack slides it out underneath.
        .zIndex(2)
    }

    private func showSidebar(_ open: Bool) {
        withAnimation(Theme.sidebarSlide) { store.sidebarOpen = open }
    }

    private var content: some View {
        VStack(spacing: 0) {
            if let compose = store.compose {
                // Writing is a page of its own, as a thread is, with its bar on top.
                ComposeView(compose: compose).id(compose.id)
            } else {
                MacTopBar(focusSearch: focusSearch, showSidebar: { showSidebar(true) })
                Rule()
                page
            }
        }
        .background(Tokens.background.color)
    }

    /// The list, and the thread open over it.
    private var page: some View {
        ZStack {
            Tokens.background.color
            // The list stays under an open thread, so going back finds it where it was left. It
            // stays through a change of mailbox too, with the empty state over it when there is
            // nothing to show, so the table isn't made anew and the page never goes blank between.
            let header = ListHeader.shows(store)
            VStack(spacing: 0) {
                // The list scrolls the whole page; only its header stays put above it.
                if header {
                    ListHeader()
                        .frame(maxWidth: Theme.cardWidth)
                        .padding(.top, Space.l)
                        .padding(.horizontal, Space.xl + 4)
                }
                ThreadListMac(
                    store: store, rows: store.visibleRows, selected: store.selected, checked: store.selection,
                    topInset: header ? 0 : Space.l, shown: store.conversation == nil, ready: store.listReady,
                    loadingMore: store.loadingMore && store.searchRows == nil
                )
                .overlay {
                    if store.listEmpty { EmptyList() }
                }
            }
            .opacity(store.conversation == nil ? 1 : 0)
            .allowsHitTesting(store.conversation == nil)
            if let conversation = store.conversation {
                ThreadScreen(conversation: conversation)
            }
        }
    }
}

/// Search and compose above the list; back and the thread's actions above a thread.
struct MacTopBar: View {
    @Environment(MailStore.self) private var store
    let focusSearch: Int
    let showSidebar: () -> Void
    @Environment(\.pastWindowButtons) private var pastWindowButtons
    @State private var query = ""

    var body: some View {
        Group {
            if store.conversation == nil, !store.selection.isEmpty {
                HeaderBar {
                    menu
                    SelectionBar()
                } trailing: {
                    compose
                }
            } else {
                HeaderBar {
                    menu
                    if store.conversation != nil {
                        IconButton(symbol: .arrowLeft, help: "Back (Esc)") { store.close() }
                    } else {
                        SearchField(
                            placeholder: "Search", text: $query, style: .bar, autofocus: false, focusTrigger: focusSearch,
                            submit: { store.search(query) }, escape: store.endSearch, clear: store.endSearch
                        )
                        .frame(maxWidth: 240)
                    }
                } center: {
                    if let open = store.conversation {
                        ThreadActions(thread: open.id)
                    } else {
                        MailboxTabs()
                    }
                } trailing: {
                    compose
                }
            }
        }
        .padding(.leading, pastWindowButtons)
        .onAppear { query = store.searchQuery }
        .onChange(of: query) { _, text in store.search(text) }
        .onChange(of: store.searchQuery) { _, text in if text.isEmpty { query = "" } }
    }

    private var menu: some View {
        IconButton(symbol: .menu, help: "Mailboxes", circled: false, action: showSidebar)
    }

    private var compose: some View {
        ActionButton(title: "Compose", symbol: .squarePen, variant: .outline, action: store.newMessage)
    }
}
#endif
