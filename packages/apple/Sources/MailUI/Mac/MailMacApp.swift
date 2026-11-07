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
                .onAppear {
                    store.appearance.apply()
                    delegate.store = store
                    store.start(defaultServer: defaultServer)
                }
                .onOpenURL { url in Task { await store.finishSignIn(url) } }
        }
        .windowToolbarStyle(.unified(showsTitle: false))
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

/// The window, around the pages. Each page puts its first button in the window's toolbar, beside
/// the window's buttons, where the system gives it its clicks; what follows it on the bar is ours,
/// drawn under the toolbar. This view reads nothing of the mail, so a sync doesn't redraw it.
struct MacRoot: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        MacPages()
            .comboboxHost()
            .toolbar {
                // Until there is a page, an item no one sees, so the window's buttons sit where the
                // pages will have them.
                if !store.booted || !store.signedIn {
                    ToolbarItem(placement: .navigation) {
                        Color.clear.frame(width: 1, height: 1).allowsHitTesting(false)
                    }
                    .withoutSystemGlass()
                }
            }
            .frame(minWidth: minWidth, minHeight: 520)
            .background(WindowWidening(toWidth: minWidth))
            .ignoresSafeArea()
            .windowToolbarUnderTopBar()
            .mailSheets(store)
    }

    /// A sidebar kept open takes its width from the window, never from the mail beside it.
    private var minWidth: CGFloat {
        860 + (store.keepSidebar && store.sidebarOpen ? Theme.sidebarWidth : 0)
    }
}

/// The mail, the sidebar over it and the settings; onboarding until an account is added.
private struct MacPages: View {
    @Environment(MailStore.self) private var store
    @State private var focusSearch = 0
    @State private var farDown = false
    @State private var toTop = 0

    var body: some View {
        @Bindable var store = store
        if !store.booted {
            // The page's colour alone, until the core says where the app was left.
            Tokens.background.color
        } else if !store.signedIn {
            OnboardingView().overlay(alignment: .bottom) { ToastView() }
        } else {
            ZStack(alignment: .leading) {
                // The mail stays under the settings, so going back finds it as it was left.
                HStack(spacing: 0) {
                    if store.settings == nil, store.keepSidebar, store.sidebarOpen { panel }
                    content
                }
                .opacity(store.settings == nil ? 1 : 0)
                .allowsHitTesting(store.settings == nil)
                if store.settings == nil, !store.keepSidebar { sidebar }
                if let section = store.settings { SettingsRoute(section: section) }
            }
            .overlay(alignment: .bottom) {
                VStack(spacing: 0) {
                    ToastView()
                    UpdateBanner(updater: store.updater)
                }
            }
            .background(KeyHandler(store: store, focusSearch: { focusSearch += 1 }, sidebarOpen: $store.sidebarOpen))
        }
    }

    /// Over the page, without moving it; a click beside it closes it.
    @ViewBuilder private var sidebar: some View {
        // Always there and only faded, so it stops taking clicks the moment the sidebar closes:
        // a removed view keeps them until its spring has fully settled.
        Tokens.overlay.opacity(Opacity.overlay).color
            .contentShape(Rectangle())
            .onTapGesture { showSidebar(false) }
            .opacity(store.sidebarOpen ? 1 : 0)
            .allowsHitTesting(store.sidebarOpen)
            .zIndex(1)
        if store.sidebarOpen { panel }
    }

    private var panel: some View {
        VStack(spacing: 0) {
            // Under the window's buttons.
            Color.clear.frame(height: Theme.barHeight)
            SidebarView(
                showSettings: {
                    if !store.keepSidebar { showSidebar(false) }
                    store.openSettings()
                },
                picked: { _ in if !store.keepSidebar { showSidebar(false) } }
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
                MacTopBar(focusSearch: focusSearch, toggleSidebar: { showSidebar(!store.sidebarOpen) })
                Rule()
                page
            }
        }
        .background(Tokens.background.color)
    }

    /// The list, and the thread open over it, or beside it where the page is wide enough.
    private var page: some View {
        ZStack {
            Tokens.background.color
            // The list stays under an open thread, so going back finds it where it was left. It
            // stays through a change of mailbox too, with the empty state over it when there is
            // nothing to show, so the table isn't made anew and the page never goes blank between.
            let header = ListHeader.shows(store)
            let beside = store.besideList
            let listShown = beside || store.conversation == nil
            HStack(spacing: 0) {
                VStack(spacing: 0) {
                    // The list scrolls the whole page; only its header stays put above it.
                    if header {
                        // Beside the open mail, edge to edge as on the phones.
                        if beside {
                            ListHeader()
                        } else {
                            ListHeader(edgeInset: 0)
                                .frame(maxWidth: Theme.cardWidth)
                                .padding(.horizontal, Space.xl + 4 + Theme.macSelectGutter)
                        }
                    }
                    ThreadListMac(
                        store: store, rows: store.visibleRows, selected: store.selected, checked: store.selection,
                        hoverCircles: store.selectCircles,
                        topInset: beside ? 0 : header ? Space.s : Space.l, shown: listShown, compact: beside, ready: store.listReady,
                        loadingMore: store.loadingMore && store.searchRows == nil,
                        farDown: { farDown = $0 }, toTop: toTop
                    )
                    .overlay {
                        if store.listEmpty { EmptyList() }
                    }
                    .overlay(alignment: .top) { ToTop(shown: farDown) { toTop += 1 } }
                }
                .frame(width: beside ? Theme.listColumn : nil)
                .opacity(listShown ? 1 : 0)
                .allowsHitTesting(listShown)
                if beside {
                    Rule(vertical: true)
                    pane
                }
            }
            if !beside, let conversation = store.conversation {
                ThreadScreen(conversation: conversation)
            }
        }
        .onGeometryChange(for: Bool.self, of: { $0.size.width >= Theme.readingFrom }) { store.pageWide = $0 }
    }

    /// The open thread beside the list, with its actions above it.
    @ViewBuilder private var pane: some View {
        if let conversation = store.conversation {
            VStack(spacing: 0) {
                ThreadActions(thread: conversation.id)
                    .frame(maxWidth: .infinity)
                    .frame(height: Theme.barHeight)
                    .rule(.bottom)
                ThreadScreen(conversation: conversation)
            }
        } else {
            EmptyState(symbol: .mailOpen, message: "No conversation open")
        }
    }
}

/// The mailbox's name, its tabs, search and compose above the list; back and the thread's
/// actions above a thread over the list. The menu, and back beside it, are the toolbar's.
struct MacTopBar: View {
    @Environment(MailStore.self) private var store
    let focusSearch: Int
    let toggleSidebar: () -> Void
    @State private var query = ""
    /// Where the menu button's padded frame ends, from the window's left edge, as the toolbar
    /// reports it; a usual value until it has.
    @State private var pastMenu: CGFloat = 133

    /// The thread over the list; one beside it leaves the bar to the mailbox.
    private var open: Conversation? {
        store.besideList ? nil : store.conversation
    }

    var body: some View {
        Group {
            if open == nil, !store.selection.isEmpty {
                HeaderBar(leadingInset: leadingInset) {
                    SelectionCount()
                } trailing: {
                    SelectionActions()
                }
            } else {
                HeaderBar(leadingInset: leadingInset) {
                    if open == nil {
                        MailboxTitle()
                    }
                } center: {
                    if let open {
                        ThreadActions(thread: open.id)
                    } else {
                        MailboxTabs()
                    }
                } trailing: {
                    if open == nil {
                        SearchField(
                            placeholder: "Search", text: $query, style: .bar, autofocus: false, focusTrigger: focusSearch,
                            submit: { store.search(query) }, escape: store.endSearch, clear: store.endSearch
                        )
                        .frame(width: 160)
                    }
                    compose
                }
            }
        }
        .toolbar {
            // The mail stays under the settings, which have their own.
            if store.settings == nil {
                ToolbarItem(placement: .navigation) {
                    HStack(spacing: 0) {
                        IconButton(symbol: .menu, help: "Mailboxes", circled: false, action: toggleSidebar)
                            .background(ToolbarEdge { pastMenu = $0 })
                        // Over the open sidebar, only the menu, which closes it.
                        if open != nil, !store.sidebarOpen || store.keepSidebar {
                            IconButton(symbol: .arrowLeft, help: "Back (Esc)") { store.close() }
                        }
                    }
                    // AppKit resizes the item at once; a fading back button would be centred in
                    // it meanwhile and push the menu sideways.
                    .transaction { $0.animation = nil }
                }
                .withoutSystemGlass()
            }
        }
        .onAppear { query = store.searchQuery }
        .onChange(of: query) { _, text in store.search(text) }
        .onChange(of: store.searchQuery) { _, text in if text.isEmpty { query = "" } }
    }

    /// Past the toolbar's buttons, in the leading side alone, so the center stays in the middle of the
    /// window. The title starts where back's circle does, half a button gap past the menu's margin.
    /// A sidebar kept open holds the buttons, so the bar starts at its own edge.
    private var leadingInset: CGFloat {
        guard !(store.keepSidebar && store.sidebarOpen) else { return 0 }
        return pastMenu + Theme.buttonGap / 2 - Space.l
    }

    /// Its words when there is room, its icon alone when there is not.
    private var compose: some View {
        ViewThatFits(in: .horizontal) {
            ActionButton(title: "Compose", symbol: .squarePen, variant: .outline, action: store.newMessage)
            ActionButton(title: "Compose", symbol: .squarePen, variant: .outline, iconOnly: true, action: store.newMessage)
        }
    }
}
#endif
