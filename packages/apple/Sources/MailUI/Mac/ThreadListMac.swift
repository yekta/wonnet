#if os(macOS)
import AppKit
import SwiftUI

/// The thread list on the Mac: a table that only makes the rows on screen, each drawn in one
/// pass. It spans the window, so the whole page scrolls, and draws its rows on the page in the
/// centred column. Hovering a row shows what can be done to it, as Newton did, and the circle in
/// the gutter before it that selects it. While rows are selected, every row shows its circle and
/// a click selects instead of opening. ⌘-click picks rows, Shift-click picks the rows up to one.
/// A right-click offers what can be done to the row, or to the selection it is in.
struct ThreadListMac: NSViewRepresentable {
    let store: MailStore
    let rows: [ThreadRow]
    let selected: String?
    let checked: Set<String>
    /// Whether hovering a row shows its circle; while rows are selected, every row shows it anyway.
    let hoverCircles: Bool
    let topInset: CGFloat
    /// False under an open thread: the list keeps its place, and gives up the keyboard.
    let shown: Bool
    /// False while the rows are the mailbox before; when they are the new one's, the list goes to the top.
    let ready: Bool
    /// The next page is being read: a row at the foot says so.
    let loadingMore: Bool
    /// Told whether the list is scrolled past its first page.
    let farDown: (Bool) -> Void
    /// Goes back to the top each time it changes.
    let toTop: Int

    func makeCoordinator() -> Coordinator { Coordinator(store: store, toTop: toTop) }

    func makeNSView(context: Context) -> NSScrollView {
        let table = HoverTableView()
        table.addTableColumn(NSTableColumn(identifier: NSUserInterfaceItemIdentifier("thread")))
        table.headerView = nil
        table.rowHeight = Theme.macRowHeight
        table.intercellSpacing = .zero
        table.backgroundColor = .clear
        table.selectionHighlightStyle = .none
        table.style = .plain
        table.dataSource = context.coordinator
        table.delegate = context.coordinator
        table.target = context.coordinator
        table.action = #selector(Coordinator.clicked)
        table.focusRingType = .none
        table.menuForRow = { [weak coordinator = context.coordinator] index in coordinator?.menu(for: index) }

        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.automaticallyAdjustsContentInsets = false
        pad(scroll)
        scroll.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            context.coordinator, selector: #selector(Coordinator.scrolled), name: NSView.boundsDidChangeNotification, object: scroll.contentView
        )
        context.coordinator.table = table
        context.coordinator.keeper = ScrollKeeper(restore: store.listOffset, report: store.noteScroll)
        context.coordinator.keeper?.attach(scroll)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        if scroll.contentInsets.top != topInset { pad(scroll) }
        if scroll.isHidden == shown { scroll.isHidden = !shown }
        context.coordinator.farDown = farDown
        context.coordinator.update(rows: rows, selected: selected, checked: checked, hoverCircles: hoverCircles, ready: ready, loadingMore: loadingMore)
        context.coordinator.goToTop(toTop)
    }

    /// Pads the rows, not the scroll view, so the padding scrolls with them and the scroller
    /// runs the full height.
    private func pad(_ scroll: NSScrollView) {
        scroll.contentInsets = NSEdgeInsets(top: topInset, left: 0, bottom: Self.bottomInset, right: 0)
        scroll.scrollerInsets = NSEdgeInsets(top: -topInset, left: 0, bottom: -Self.bottomInset, right: 0)
    }

    private static let bottomInset = Space.l

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: Coordinator) {
        NotificationCenter.default.removeObserver(coordinator)
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        let store: MailStore
        weak var table: HoverTableView?
        var farDown: (Bool) -> Void = { _ in }
        private var isFarDown = false
        private var toTop: Int
        private var rows: [ThreadRow] = []
        private var texts: [String: (ThreadRow, RowText)] = [:]
        private var selected: String?
        private var checked: Set<String> = []
        private var hoverCircles = true
        private var ready = true
        private var loadingMore = false
        private var prefetch: DispatchWorkItem?
        /// The row last clicked: it is where the pointer is, so the list doesn't move to show it.
        private var clickedRow: String?
        /// Keeps the list where it was left, across the window and the app being made anew.
        var keeper: ScrollKeeper?

        init(store: MailStore, toTop: Int) {
            self.store = store
            self.toTop = toTop
            // The row selected when the list is made isn't a move to show: the keeper puts the
            // list back where it was scrolled.
            selected = store.selected
        }

        func update(rows: [ThreadRow], selected: String?, checked: Set<String>, hoverCircles: Bool, ready: Bool, loadingMore: Bool) {
            guard let table else { return }
            let moved = selected != self.selected
            let picked = checked != self.checked || hoverCircles != self.hoverCircles
            self.hoverCircles = hoverCircles
            let switched = ready && !self.ready
            let changed = Self.changes(from: self.rows, to: rows)
            self.rows = rows
            self.selected = selected
            self.checked = checked
            self.ready = ready
            let footChanged = loadingMore != self.loadingMore
            self.loadingMore = loadingMore
            if let changed {
                for index in changed {
                    guard let view = table.rowView(atRow: index, makeIfNecessary: false) as? ThreadRowView else { continue }
                    configure(view, at: index)
                }
                if footChanged, loadingMore {
                    table.insertRows(at: [rows.count], withAnimation: [])
                } else if footChanged {
                    table.removeRows(at: [rows.count], withAnimation: [])
                }
            } else {
                table.reloadData()
            }
            if switched { keeper?.top() }
            if changed != nil, moved || picked {
                table.enumerateAvailableRowViews { view, _ in
                    guard let view = view as? ThreadRowView, let id = view.row?.id else { return }
                    view.isCurrent = id == selected
                    view.isChecked = checked.contains(id)
                    view.isSelecting = !checked.isEmpty
                    view.hoverCircle = hoverCircles
                }
            }
            if moved, let selected, selected != clickedRow, store.conversation == nil,
               let index = rows.firstIndex(where: { $0.id == selected }) {
                table.scrollRowToVisible(index)
            }
            if moved { clickedRow = nil }
            if changed != [] { schedulePrefetch() }
        }

        /// The rows that changed when the list holds the same threads in the same order; nil
        /// when it doesn't, and the whole table is read again.
        private static func changes(from old: [ThreadRow], to new: [ThreadRow]) -> IndexSet? {
            guard old != new else { return [] }
            guard old.count == new.count else { return nil }
            var changed = IndexSet()
            for index in new.indices where old[index] != new[index] {
                guard old[index].id == new[index].id else { return nil }
                changed.insert(index)
            }
            return changed
        }

        private func text(for row: ThreadRow) -> RowText {
            if let (kept, text) = texts[row.id], kept == row { return text }
            let text = RowText(row)
            texts[row.id] = (row, text)
            return text
        }

        private func configure(_ view: ThreadRowView, at index: Int) {
            let row = rows[index]
            view.configure(
                row: row, text: text(for: row), current: row.id == selected, checked: checked.contains(row.id),
                selecting: !checked.isEmpty, hoverCircle: hoverCircles, hovering: table?.hovered == index,
                selectHelp: store.keyHelp("Select", "select")
            )
        }

        func numberOfRows(in tableView: NSTableView) -> Int { rows.count + (loadingMore ? 1 : 0) }

        func tableView(_ tableView: NSTableView, rowViewForRow index: Int) -> NSTableRowView? {
            guard index < rows.count else {
                return tableView.makeView(withIdentifier: LoadingMoreRowView.identifier, owner: nil) as? LoadingMoreRowView ?? LoadingMoreRowView()
            }
            let identifier = NSUserInterfaceItemIdentifier("row")
            let view = tableView.makeView(withIdentifier: identifier, owner: nil) as? ThreadRowView ?? ThreadRowView()
            view.identifier = identifier
            configure(view, at: index)
            if index > rows.count - 30 { DispatchQueue.main.async { self.store.loadMore() } }
            return view
        }

        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? { nil }

        @objc func scrolled() {
            schedulePrefetch()
            guard let scroll = table?.enclosingScrollView else { return }
            let clip = scroll.contentView.bounds
            let far = clip.minY + scroll.contentInsets.top > clip.height
            guard far != isFarDown else { return }
            isFarDown = far
            // Scrolls also come while SwiftUI updates the table, when its state can't change.
            Task { self.farDown(far) }
        }

        func goToTop(_ request: Int) {
            guard request != toTop, let scroll = table?.enclosingScrollView else { return }
            toTop = request
            let top = NSPoint(x: scroll.contentView.bounds.minX, y: -scroll.contentInsets.top)
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.3
                scroll.contentView.animator().setBoundsOrigin(top)
            } completionHandler: {
                scroll.reflectScrolledClipView(scroll.contentView)
            }
        }

        /// Asks for the bodies of the rows on screen once the list stops moving.
        private func schedulePrefetch() {
            prefetch?.cancel()
            let work = DispatchWorkItem { [weak self] in
                MainActor.assumeIsolated {
                    guard let self, let table = self.table else { return }
                    let visible = table.rows(in: table.visibleRect)
                    let range = max(visible.location, 0)..<min(visible.location + visible.length, self.rows.count)
                    guard !range.isEmpty else { return }
                    self.store.prefetch(self.rows[range].map(\.id))
                }
            }
            prefetch = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: work)
        }

        /// What a right-click on the row offers: for the selection when the row is in it.
        func menu(for index: Int) -> NSMenu? {
            guard rows.indices.contains(index) else { return nil }
            let row = rows[index]
            let threads = store.targets(for: row.id)
            let many = threads.count > 1
            let groups: [[ThreadCommand]] = [
                many ? [] : [.reply, .forward],
                [.archive, .trash, .snooze, .read, .star],
                [.label, .move],
            ]
            let menu = NSMenu()
            if !many, row.draftId == nil {
                menu.addItem(ActionMenuItem(title: "Open") { [store] in store.open(row.id) })
                menu.addItem(.separator())
            }
            for group in groups {
                let commands = store.applicable(group, to: threads)
                guard !commands.isEmpty else { continue }
                for command in commands {
                    menu.addItem(ActionMenuItem(title: command.title(store, threads), symbol: command.symbol) { [store] in
                        store.run(command, on: threads)
                    })
                }
                menu.addItem(.separator())
            }
            if many {
                menu.addItem(ActionMenuItem(title: "Clear the selection", symbol: .x) { [store] in store.clearSelection() })
            } else {
                let picked = checked.contains(row.id)
                menu.addItem(ActionMenuItem(title: picked ? "Deselect" : "Select", symbol: .circleCheck) { [store] in
                    store.toggleSelection(row.id)
                })
            }
            return menu
        }

        @objc func clicked() {
            guard let table, table.clickedRow >= 0, table.clickedRow < rows.count else { return }
            let row = rows[table.clickedRow]
            guard let view = table.rowView(atRow: table.clickedRow, makeIfNecessary: false) as? ThreadRowView,
                  let event = NSApp.currentEvent
            else { return }
            let point = view.convert(event.locationInWindow, from: nil)
            guard view.reach.contains(point) else { return }
            let hit = view.hit(point)
            if event.modifierFlags.contains(.shift) {
                store.extendSelection(to: row.id)
                return
            }
            if hit == .select || event.modifierFlags.contains(.command) || (!checked.isEmpty && hit != .star) {
                store.toggleSelection(row.id)
                return
            }
            let threads = store.targets(for: row.id)
            switch hit {
            case .star: store.run(.star, on: threads)
            case .archive: store.run(.archive, on: threads)
            case .trash: store.run(.trash, on: threads)
            case .snooze: store.run(.snooze, on: threads)
            case .read: store.run(.read, on: threads)
            case .select, .none:
                clickedRow = row.id
                store.clearSelection()
                store.open(row.id)
            }
        }
    }
}

/// The row at the foot of the list while the next page is being read.
final class LoadingMoreRowView: NSTableRowView {
    static let identifier = NSUserInterfaceItemIdentifier("loadingMore")

    init() {
        super.init(frame: .zero)
        identifier = Self.identifier
        let host = NSHostingView(rootView: LoadingMore())
        host.frame = bounds
        host.autoresizingMask = [.width, .height]
        addSubview(host)
    }

    required init?(coder: NSCoder) { nil }

    override func drawSelection(in dirtyRect: NSRect) {}
}

/// A menu item that does what it is given.
final class ActionMenuItem: NSMenuItem {
    private let perform: () -> Void

    init(title: String, symbol: Symbol? = nil, perform: @escaping () -> Void) {
        self.perform = perform
        super.init(title: title, action: #selector(run), keyEquivalent: "")
        target = self
        guard let symbol, let icon = NSImage.symbol(symbol, side: 15).copy() as? NSImage else { return }
        icon.isTemplate = true
        image = icon
    }

    required init(coder: NSCoder) {
        fatalError("Not made from a coder")
    }

    @objc private func run() {
        perform()
    }
}

/// Tells its rows when the pointer is over them, and asks for a row's menu on a right-click.
final class HoverTableView: NSTableView {
    var menuForRow: (Int) -> NSMenu? = { _ in nil }

    override func menu(for event: NSEvent) -> NSMenu? {
        let point = convert(event.locationInWindow, from: nil)
        let index = row(at: point)
        guard index >= 0, let view = rowView(atRow: index, makeIfNecessary: false) as? ThreadRowView,
              view.reach.contains(convert(point, to: view))
        else { return nil }
        return menuForRow(index)
    }

    var hovered = -1 {
        didSet {
            guard hovered != oldValue else { return }
            for index in [oldValue, hovered] where index >= 0 && index < numberOfRows {
                (rowView(atRow: index, makeIfNecessary: false) as? ThreadRowView)?.hovering = index == hovered
            }
        }
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self))
    }

    override func mouseMoved(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let index = row(at: point)
        guard index >= 0, let view = rowView(atRow: index, makeIfNecessary: false) as? ThreadRowView,
              view.reach.contains(convert(point, to: view))
        else {
            hovered = -1
            return
        }
        hovered = index
        view.hoveredHit = view.hit(convert(point, to: view))
    }

    override func mouseExited(with event: NSEvent) {
        hovered = -1
    }
}

/// One thread on one line: the account's bar, unread dot, senders, subject and snippet, the star
/// if starred, date; before it, in the gutter, the circle that selects it.
final class ThreadRowView: NSTableRowView, NSViewToolTipOwner {
    enum Hit {
        case select, star, archive, trash, snooze, read, none
    }

    private(set) var row: ThreadRow?
    private var text: RowText?
    var isCurrent = false { didSet { if isCurrent != oldValue { needsDisplay = true } } }
    var isChecked = false { didSet { if isChecked != oldValue { needsDisplay = true } } }
    /// Rows are selected: every row shows its circle, and hovering one doesn't offer its actions.
    var isSelecting = false { didSet { if isSelecting != oldValue { needsDisplay = true } } }
    /// Hovering the row shows its circle. Without it, the gutter is left alone until rows are selected.
    var hoverCircle = true { didSet { if hoverCircle != oldValue { needsDisplay = true } } }
    private var selectHelp = "Select"
    var hovering = false {
        didSet {
            guard hovering != oldValue else { return }
            if !hovering { hoveredHit = .none }
            needsDisplay = true
        }
    }
    /// The action the pointer is over, lit as a button would be.
    var hoveredHit = Hit.none { didSet { if hoveredHit != oldValue { needsDisplay = true } } }

    private static let sendersX: CGFloat = 36
    private static let sendersWidth: CGFloat = 190
    private static let iconSide: CGFloat = 16
    private static let starSide: CGFloat = 14
    /// The least room the date takes, so the stars and the line's ends line up from row to row.
    private static let dateColumn = ceil(NSAttributedString(string: "Yesterday", attributes: [.font: Platform.face(RowText.dateSize, .semibold)]).size().width)

    func configure(row: ThreadRow, text: RowText, current: Bool, checked: Bool, selecting: Bool, hoverCircle: Bool, hovering: Bool, selectHelp: String) {
        self.row = row
        self.text = text
        self.isCurrent = current
        self.isChecked = checked
        self.isSelecting = selecting
        self.hoverCircle = hoverCircle
        self.hovering = hovering
        self.selectHelp = selectHelp
        needsDisplay = true
    }

    override var isFlipped: Bool { true }

    /// Where the row is drawn: the column's width, centred, with the page's margin and a gutter
    /// either side.
    var column: NSRect {
        let width = max(min(bounds.width - 48 - 2 * Theme.macSelectGutter, Theme.cardWidth), 0)
        return NSRect(x: ((bounds.width - width) / 2).rounded(), y: 0, width: width, height: bounds.height)
    }

    /// The gutter before the column, where the circle that selects the row is.
    private var gutter: NSRect {
        NSRect(x: column.minX - Theme.macSelectGutter, y: 0, width: Theme.macSelectGutter, height: bounds.height)
    }

    /// Whether the circle in the gutter can be clicked: it is shown on hover, or rows are selected.
    private var circleShown: Bool { hoverCircle || isSelecting }

    /// Where the row answers the pointer: the column, and the gutter before it while its circle is there.
    var reach: NSRect { circleShown ? column.union(gutter) : column }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        removeAllToolTips()
        addToolTip(gutter, owner: self, userData: nil)
    }

    func view(_ view: NSView, stringForToolTip tag: NSView.ToolTipTag, point: NSPoint, userData data: UnsafeMutableRawPointer?) -> String {
        guard circleShown else { return "" }
        return isChecked ? "Deselect" : selectHelp
    }

    /// The row's face: the column, with half the gap between rows above and below.
    private var face: NSRect {
        column.insetBy(dx: 0, dy: Theme.rowGap / 2)
    }

    /// Where the date and the hovered actions end.
    private var trailing: CGFloat {
        face.maxX - Space.l
    }

    /// The hovered actions, from the right.
    private var actions: [(Hit, Symbol, NSRect)] {
        guard let row else { return [] }
        let kinds: [(Hit, Symbol)] = row.draftId != nil
            ? [(.trash, .trash)]
            : [(.star, row.starred ? .starFilled : .star), (.read, row.unread ? .mailOpen : .mail), (.snooze, .clock), (.trash, .trash), (.archive, .archive)]
        return kinds.enumerated().map { index, kind in
            let x = trailing - Self.iconSide - CGFloat(index) * 30
            return (kind.0, kind.1, NSRect(x: x, y: (bounds.height - Self.iconSide) / 2, width: Self.iconSide, height: Self.iconSide))
        }
    }

    func hit(_ point: NSPoint) -> Hit {
        if circleShown, gutter.contains(point) { return .select }
        guard hovering, !isSelecting else { return .none }
        return actions.first(where: { $0.2.insetBy(dx: -7, dy: -12).contains(point) })?.0 ?? .none
    }

    override func drawBackground(in dirtyRect: NSRect) {
        let shape = NSBezierPath(roundedRect: face, xRadius: Radius.sm, yRadius: Radius.sm)
        if hovering {
            Tokens.backgroundAccentLarger.platform.setFill()
            shape.fill()
        }
        if isCurrent {
            let outline = NSBezierPath(roundedRect: face.insetBy(dx: 0.5, dy: 0.5), xRadius: Radius.sm, yRadius: Radius.sm)
            outline.lineWidth = 1
            outline.setLineDash([4, 3], count: 2, phase: 0)
            Tokens.backgroundAccentStronger.platform.setStroke()
            outline.stroke()
        }
        if isChecked {
            Tokens.primary.opacity(Opacity.colorTint).platform.setFill()
            shape.fill()
        }
        if let row {
            Theme.accountColor(row.color).platform.setFill()
            let bar = NSRect(x: column.minX + 3, y: 6, width: Theme.macAccountBarWidth, height: column.height - 12)
            let radius = Theme.macAccountBarWidth / 2
            NSBezierPath(roundedRect: bar, xRadius: radius, yRadius: radius).fill()
        }
    }

    override func drawSelection(in dirtyRect: NSRect) {}

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard let row, let text else { return }
        let left = column.minX
        let middle = bounds.height / 2
        drawSelectCircle(middle: middle)
        if row.unread {
            Tokens.primary.platform.setFill()
            NSBezierPath(ovalIn: NSRect(x: left + 18, y: middle - 3.5, width: 7, height: 7)).fill()
        }
        Self.drawLine(text.senders, x: left + Self.sendersX, width: Self.sendersWidth, middle: middle)
        var x = left + Self.sendersX + Self.sendersWidth + 14
        if row.attachment {
            Self.drawSymbol(.paperclip, in: NSRect(x: x, y: middle - 7, width: 14, height: 14), color: Tokens.mutedStrongerForeground.platform)
        }
        x += 24
        let dateX = trailing - ceil(text.date.size().width)
        let dateColumnX = min(dateX, trailing - Self.dateColumn)
        let starred = row.starred && row.draftId == nil
        let star = NSRect(x: dateColumnX - Space.m - Self.starSide, y: middle - Self.starSide / 2, width: Self.starSide, height: Self.starSide)
        let lineWidth = max((starred ? star.minX - Space.s : dateColumnX - Space.l) - x, 0)
        guard hovering, !isSelecting else {
            Self.drawLine(text.line, x: x, width: lineWidth, middle: middle)
            if starred { Self.drawSymbol(.starFilled, in: star, color: Tokens.star.platform) }
            Self.drawLine(text.date, x: dateX, width: trailing - dateX + 2, middle: middle)
            return
        }
        let actionsStart = (actions.last?.2.minX ?? trailing) - Space.l
        Self.drawLine(text.line, x: x, width: lineWidth, middle: middle, fadingBefore: actionsStart)
        for (kind, symbol, rect) in actions {
            let lit = hoveredHit == kind
            let isStar = kind == .star
            if lit { Self.drawHover(around: rect, color: isStar ? Tokens.star.opacity(Opacity.colorTint).platform : Tokens.backgroundAccentLargerStrongest.platform) }
            let color = isStar && (lit || row.starred) ? Tokens.star.platform : lit ? Tokens.foreground.platform : Tokens.mutedForeground.platform
            Self.drawSymbol(symbol, in: rect, color: color)
        }
    }

    /// The circle in the gutter: filled with a check when the row is selected, empty while it is
    /// hovered or other rows are selected, else not there.
    private func drawSelectCircle(middle: CGFloat) {
        let side = Theme.selectCircle
        let rect = NSRect(x: gutter.midX - side / 2, y: middle - side / 2, width: side, height: side)
        if isChecked {
            Tokens.primary.platform.setFill()
            NSBezierPath(ovalIn: rect).fill()
            Self.drawSymbol(.check, in: rect.insetBy(dx: 3, dy: 3), color: Tokens.primaryForeground.platform)
            return
        }
        guard (hovering && hoverCircle) || isSelecting else { return }
        let inset = Theme.selectCircleLine / 2
        let ring = NSBezierPath(ovalIn: rect.insetBy(dx: inset, dy: inset))
        ring.lineWidth = Theme.selectCircleLine
        (hoveredHit == .select ? Tokens.mutedForeground : Tokens.border).platform.setStroke()
        ring.stroke()
    }

    /// The circle a hovered icon sits on, as tall as the smallest control.
    private static func drawHover(around rect: NSRect, color: NSColor) {
        let inset = (rect.width - ControlSize.small.height) / 2
        color.setFill()
        NSBezierPath(ovalIn: rect.insetBy(dx: inset, dy: inset)).fill()
    }

    private static func drawLine(_ text: NSAttributedString, x: CGFloat, width: CGFloat, middle: CGFloat) {
        guard width > 8 else { return }
        let height = ceil(text.size().height)
        text.draw(with: NSRect(x: x, y: middle - height / 2, width: width, height: height), options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine])
    }

    /// The line laid out as at rest, faded out before `end` so the actions sit over it without moving it.
    private static func drawLine(_ text: NSAttributedString, x: CGFloat, width: CGFloat, middle: CGFloat, fadingBefore end: CGFloat) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        guard let fade = CGGradient(colorsSpace: nil, colors: [CGColor(gray: 0, alpha: 0), CGColor(gray: 0, alpha: 1)] as CFArray, locations: nil) else { return }
        context.saveGState()
        context.clip(to: NSRect(x: x, y: 0, width: width, height: middle * 2))
        context.beginTransparencyLayer(auxiliaryInfo: nil)
        drawLine(text, x: x, width: width, middle: middle)
        context.setBlendMode(.destinationOut)
        context.drawLinearGradient(fade, start: CGPoint(x: end - (Space.xxl + Space.xxs), y: 0), end: CGPoint(x: end, y: 0), options: [.drawsAfterEndLocation])
        context.endTransparencyLayer()
        context.restoreGState()
    }

    private static func drawSymbol(_ symbol: Symbol, in rect: NSRect, color: NSColor) {
        let image = NSImage.symbol(symbol, side: rect.width)
        let tinted = NSImage(size: rect.size, flipped: false) { frame in
            image.draw(in: frame)
            color.set()
            frame.fill(using: .sourceAtop)
            return true
        }
        tinted.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
    }
}
#endif
