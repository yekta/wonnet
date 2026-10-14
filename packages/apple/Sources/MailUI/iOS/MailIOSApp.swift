#if os(iOS)
import SwiftUI

/// The iOS app: a mailbox's threads, then a thread, in one navigation stack, with the mailboxes
/// kept as tabs along the bottom and every mailbox in the drawer.
public struct MailIOSApp: App {
    /// Started at launch rather than when the window shows: a push or a notification's button
    /// wakes the app in the background, with no window.
    private static let store = MailStore()
    @UIApplicationDelegateAdaptor(IOSDelegate.self) private var delegate

    public init() {
        Notifier.shared.install()
        let server = Bundle.main.object(forInfoDictionaryKey: "MailServerURL") as? String ?? "http://localhost:3000"
        Self.store.start(defaultServer: server)
    }

    public var body: some Scene {
        WindowGroup {
            IOSRoot()
                .environment(Self.store)
                .onAppear { Self.store.appearance.apply() }
                .onOpenURL { url in Task { await Self.store.finishSignIn(url) } }
        }
    }
}

/// Hears from APNs: this device's token, and the background pushes that take notifications down.
final class IOSDelegate: NSObject, UIApplicationDelegate {
    /// Asks APNs for a token once launched: asked any sooner, its answer never comes.
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        application.registerForRemoteNotifications()
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        Notifier.shared.registered(token: deviceToken)
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        Notifier.shared.failedToRegister(error)
    }

    func application(_ application: UIApplication, didReceiveRemoteNotification userInfo: [AnyHashable: Any]) async -> UIBackgroundFetchResult {
        await Notifier.shared.received(userInfo)
        return .newData
    }
}

struct IOSRoot: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        @Bindable var store = store
        Group {
            if !store.booted {
                // The page's colour alone, until the core says where the app was left.
                Tokens.background.color.ignoresSafeArea()
            } else if !store.signedIn {
                OnboardingView()
            } else {
                IOSShell(start: (store.searchRows == nil ? [] : [.search]) + (store.conversation.map { [.thread($0.id)] } ?? []))
                    .ignoresSafeArea()
            }
        }
        .overlay(alignment: .bottom) { ToastView() }
        .sheet(item: $store.compose) { compose in ComposeView(compose: compose).environment(store) }
        .mailSheets(store)
    }
}

/// A page over the mailbox: the search, or a thread.
enum Route: Hashable {
    case search
    case thread(String)
}

/// The drawer with the mailboxes, over the navigation stack of the mailbox on screen and the
/// pages opened over it, starting where the app was left.
struct IOSShell: View {
    @Environment(MailStore.self) private var store
    @State private var path: [Route]
    @State private var editing = false
    @State private var drawerOpen = false

    init(start: [Route]) {
        _path = State(initialValue: start)
    }

    var body: some View {
        DrawerView(isOpen: $drawerOpen, enabled: path.isEmpty && !editing) {
            SidebarView(showSettings: showSettings, picked: { _ in close() })
                .environment(store)
        } content: {
            NavigationStack(path: $path) {
                MailboxScreen(path: $path, editing: $editing, openDrawer: { drawerOpen = true })
                    .navigationDestination(for: Route.self) { route in
                        switch route {
                        case .search: SearchScreen(path: $path)
                        case .thread(let thread): ThreadScreenIOS(thread: thread)
                        }
                    }
            }
            .tint(Tokens.primary.color)
            .environment(store)
        }
        .onChange(of: store.requestedThread) { _, thread in
            guard let thread else { return }
            store.requestedThread = nil
            drawerOpen = false
            path = [.thread(thread)]
        }
        .onChange(of: store.baseMailbox) {
            path = []
            editing = false
        }
        .onChange(of: path) { _, path in
            // Leaving the search page ends the search.
            guard !path.contains(.search), !store.searchQuery.isEmpty else { return }
            store.endSearch()
        }
    }

    private func close() {
        drawerOpen = false
        path = []
    }

    private func showSettings() {
        drawerOpen = false
        store.openSettings()
    }
}

/// A mailbox's threads under our own bar, with Compose floating over them; in edit mode,
/// threads to act on together. Search is a page of its own.
struct MailboxScreen: View {
    @Environment(MailStore.self) private var store
    @Binding var path: [Route]
    @Binding var editing: Bool
    let openDrawer: () -> Void
    @State private var farDown = false
    @State private var toTop = 0

    var body: some View {
        // The table stays through a change of mailbox, with the empty state over it when there
        // is nothing to show, so it isn't made anew and the page never goes blank between.
        ThreadListIOS(
            store: store, rows: store.rows, editing: editing, checked: store.selection, ready: store.listReady,
            loadingMore: store.loadingMore, headerHeight: ListHeader.shows(store) ? ListHeader.height : 0,
            bottomInset: FloatingButton.side + Space.l * 2,
            refresh: store.checkMail, farDown: { farDown = $0 }, toTop: toTop,
            open: { thread in openThread(thread, path: $path, store: store) },
            header: { ListHeader().environment(store) }
        )
        .overlay {
            if store.rows.isEmpty, store.listReady { EmptyList().background(Tokens.background.color) }
        }
        .overlay(alignment: .top) { ToTop(shown: farDown) { toTop += 1 } }
        .overlay(alignment: .bottomTrailing) {
            if !editing {
                FloatingButton(symbol: .squarePen, help: "Compose", action: store.newMessage).padding(Space.l)
            }
        }
        .safeAreaInset(edge: .top, spacing: 0) { top }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if editing {
                CommandBar(threads: Array(store.selection)).disabled(store.selection.isEmpty)
            } else {
                MailboxTabBar()
            }
        }
        .toolbar(.hidden, for: .navigationBar)
        .navigationTitle(store.baseMailboxName)
        // A large title would take the list's refresh control into the hidden bar.
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: editing) { _, on in if !on { store.clearSelection() } }
    }

    @ViewBuilder private var top: some View {
        if editing { editingBar } else { bar }
    }

    private var bar: some View {
        HeaderBar(inset: Space.s) {
            IconButton(symbol: .menu, help: "Mailboxes", circled: false, action: openDrawer)
        } center: {
            MailboxTitle()
        } trailing: {
            IconButton(symbol: .search, help: "Search", circled: false) { path.append(.search) }
        }
        .topBar()
    }

    private var editingBar: some View {
        let all = store.selection.count == store.rows.count
        return HeaderBar(inset: Space.s) {
            ActionButton(title: all ? "Select None" : "Select All", variant: .ghost) { all ? store.clearSelection() : store.selectAll() }
        } center: {
            Text("\(store.selection.count) Selected").textStyle(.subheading).lineLimit(1)
        } trailing: {
            ActionButton(title: "Done", variant: .ghost) { editing = false }
        }
        .topBar()
    }
}

/// Opens a thread as a page over the list; a draft opens in compose instead.
@MainActor private func openThread(_ thread: String, path: Binding<[Route]>, store: MailStore) {
    let draft = store.row(thread)?.draftId != nil
    store.open(thread)
    if !draft { path.wrappedValue.append(.thread(thread)) }
}

/// Search as a page of its own: back and the field in the bar, the matches under them.
struct SearchScreen: View {
    @Environment(MailStore.self) private var store
    @Binding var path: [Route]
    @State private var query = ""
    @State private var farDown = false
    @State private var toTop = 0

    var body: some View {
        ThreadListIOS(
            store: store, rows: store.searchRows ?? [], editing: false, checked: [], ready: true, keepsScroll: false,
            headerHeight: 0, refresh: store.searchAgain, farDown: { farDown = $0 }, toTop: toTop,
            open: { thread in openThread(thread, path: $path, store: store) },
            header: { EmptyView() }
        )
        .overlay {
            if store.searchRows?.isEmpty == true { EmptyList().background(Tokens.background.color) }
        }
        .overlay(alignment: .top) { ToTop(shown: farDown) { toTop += 1 } }
        .safeAreaInset(edge: .top, spacing: 0) {
            HeaderBar {
                IconButton(symbol: .arrowLeft, help: "Back", circled: false) { path.removeAll { $0 == .search } }
                SearchField(placeholder: "Search", text: $query, style: .bar, submit: { store.search(query) }, clear: store.endSearch)
            } trailing: {
                EmptyView()
            }
            .topBar()
        }
        .toolbar(.hidden, for: .navigationBar)
        .navigationTitle("Search")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { query = store.searchQuery }
        .onChange(of: query) { _, text in store.search(text) }
    }
}

extension View {
    /// A bar of ours at the top of a page: on the page's colour, up to the top of the screen, with
    /// a hairline under it.
    func topBar() -> some View {
        background(Tokens.background.color.ignoresSafeArea(edges: .top)).rule(.bottom)
    }
}

/// The mailboxes kept as tabs, along the bottom. One held and dragged moves; the order is the
/// `tabs` preference, so every device shows the same.
struct MailboxTabBar: View {
    @Environment(MailStore.self) private var store

    var body: some View {
        if !store.tabs.isEmpty {
            BottomTabs(
                tabs: store.tabs, selected: store.baseMailbox,
                symbol: { Symbol.named($0.symbol) }, title: { store.tabName($0, shownIn: store.listAccount) }, count: \.unread,
                pick: { store.select(mailbox: $0.id) }, reorder: { store.setTabs($0.map(store.savedTab)) }
            )
        }
    }
}

/// What can be done to threads along the bottom: the usual few spread across a bar of glass,
/// with menus for the rest, and Reply on a circle of its own beside it.
struct CommandBar: View {
    @Environment(MailStore.self) private var store
    let threads: [String]
    var reply = false

    var body: some View {
        HStack(spacing: Space.s) {
            GlassBar {
                if store.applicable([.archive], to: threads).isEmpty {
                    IconButton(symbol: .trash, help: "Delete", circled: false, strong: true) { store.run(.trash, on: threads) }
                } else {
                    commands
                }
            }
            if reply {
                GlassCircle(symbol: .reply, help: "Reply") { store.run(.reply, on: threads) }
            }
        }
        .padding(.horizontal, Space.l)
        .padding(.bottom, Space.s)
    }

    @ViewBuilder private var commands: some View {
        ForEach([ThreadCommand.archive, .trash, .snooze]) { command in
            IconButton(symbol: command.symbol, help: command.title(store, threads), circled: false, strong: true) { store.run(command, on: threads) }
        }
        IconMenu(symbol: .tag, help: "Label", circled: false, strong: true) {
            ThreadCommandButtons(commands: [.label, .move], threads: threads)
        }
        IconMenu(symbol: .ellipsis, help: "More", circled: false, strong: true) {
            ThreadCommandButtons(commands: [.read] + ThreadCommand.more.filter { threads.count == 1 || $0 != .print }, threads: threads)
            if reply {
                Divider()
                ThreadCommandButtons(commands: [.replyAll, .forward], threads: threads)
            }
        }
    }
}

/// A thread, with what can be done to it along the bottom. A swipe across it turns to the
/// thread beside it in the list.
struct ThreadScreenIOS: View {
    private static let goingOut = Animation.easeIn(duration: 0.15)
    private static let comingIn = Animation.easeOut(duration: 0.2)

    @Environment(MailStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    /// The thread opened, then the ones turned to.
    @State private var thread: String
    /// How far a swipe has the page aside.
    @State private var slide: CGFloat = 0
    @State private var width: CGFloat = 0

    init(thread: String) {
        _thread = State(initialValue: thread)
    }

    var body: some View {
        Group {
            if let conversation = store.conversation, conversation.id == thread {
                ThreadScreen(conversation: conversation)
            } else {
                Spinner().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .offset(x: slide)
        .background(Tokens.background.color)
        .gesture(ThreadSwipe(canTurn: { store.sibling($0) != nil }, moved: follow, released: release))
        .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
        .navigationBarTitleDisplayMode(.inline)
        .safeAreaInset(edge: .bottom, spacing: 0) { CommandBar(threads: [thread], reply: true) }
        .onChange(of: store.conversation?.id) { _, open in
            guard let open else { return dismiss() }
            thread = open
        }
        .onDisappear {
            if store.conversation?.id == thread { store.close() }
        }
    }

    /// The page follows the finger, and gives only a little toward a side with no thread.
    private func follow(_ distance: CGFloat) {
        slide = store.sibling(distance < 0 ? 1 : -1) == nil ? distance * 0.15 : distance
    }

    /// Turns once the swipe went far or fast enough that way, else the page goes back.
    private func release(_ distance: CGFloat, _ speed: CGFloat) {
        let step = distance < 0 ? 1 : -1
        let far = abs(distance) > width * 0.3 || (abs(speed) > 400 && (speed < 0) == (distance < 0))
        guard distance != 0, far, store.sibling(step) != nil else {
            withAnimation(Self.comingIn) { slide = 0 }
            return
        }
        withAnimation(Self.goingOut) {
            slide = -CGFloat(step) * width
        } completion: {
            thread = store.turn(step) ?? thread
            slide = CGFloat(step) * width
            // The next page starts from the other side once it is there.
            DispatchQueue.main.async {
                withAnimation(Self.comingIn) { slide = 0 }
            }
        }
    }
}
#endif
