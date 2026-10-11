#if os(iOS)
import SwiftUI
import UIKit

/// The thread list on iOS: a table that only makes the rows on screen. A swipe to the left does
/// what the settings pick (`RowSwiper`); one to the right opens the drawer, unless a row's
/// commands are out, when it puts them away. A pull from the top refreshes. In edit mode, rows
/// are picked.
struct ThreadListIOS<Header: View>: UIViewRepresentable {
    let store: MailStore
    let rows: [ThreadRow]
    let editing: Bool
    let checked: Set<String>
    /// False while the rows are the mailbox before; when they are the new one's, the list goes to the top.
    let ready: Bool
    /// Whether the list comes back to where it was left; the search's results don't.
    var keepsScroll = true
    /// The next page is being read: the foot of the list says so.
    var loadingMore = false
    /// Scrolls with the rows, at their head; 0 hides it.
    let headerHeight: CGFloat
    /// Room under the last row, for what floats over the list.
    var bottomInset: CGFloat = 0
    /// What a pull from the top does; the control spins until it returns. None hides it.
    var refresh: (() async -> Void)?
    /// Told whether the list is scrolled past its first page.
    var farDown: (Bool) -> Void = { _ in }
    /// Goes back to the top each time it changes.
    var toTop = 0
    let open: (String) -> Void
    @ViewBuilder let header: () -> Header

    func makeCoordinator() -> Coordinator { Coordinator(store: store, open: open, toTop: toTop) }

    func makeUIView(context: Context) -> UITableView {
        let table = ThreadTable(frame: .zero, style: .plain)
        table.register(ThreadCell.self, forCellReuseIdentifier: ThreadCell.identifier)
        table.rowHeight = Theme.iosRowHeight
        table.separatorStyle = .none
        table.backgroundColor = Tokens.background.platform
        table.tableFooterView = context.coordinator.foot
        table.contentInset.bottom = bottomInset
        table.verticalScrollIndicatorInsets.bottom = bottomInset
        table.allowsMultipleSelectionDuringEditing = true
        table.dataSource = context.coordinator
        table.delegate = context.coordinator
        if refresh != nil {
            let control = UIRefreshControl()
            control.tintColor = Tokens.mutedForeground.platform
            control.addTarget(context.coordinator, action: #selector(Coordinator.pulled(_:)), for: .valueChanged)
            table.refreshControl = control
        }
        context.coordinator.table = table
        let swiper = RowSwiper(table: table, store: store)
        context.coordinator.swiper = swiper
        table.swiper = swiper
        let head = UIHostingController(rootView: header())
        head.view.backgroundColor = .clear
        head.safeAreaRegions = []
        context.coordinator.head = head
        if keepsScroll {
            context.coordinator.keeper = ScrollKeeper(restore: store.listOffset, report: store.noteScroll)
            context.coordinator.keeper?.attach(table)
        }
        return table
    }

    func updateUIView(_ table: UITableView, context: Context) {
        context.coordinator.open = open
        context.coordinator.refresh = refresh
        context.coordinator.farDown = farDown
        context.coordinator.head?.rootView = header()
        context.coordinator.update(rows: rows, editing: editing, checked: checked, ready: ready, headerHeight: headerHeight, loadingMore: loadingMore)
        context.coordinator.goToTop(toTop)
    }

    @MainActor
    final class Coordinator: NSObject, UITableViewDataSource, UITableViewDelegate {
        let store: MailStore
        var open: (String) -> Void
        var refresh: (() async -> Void)?
        var farDown: (Bool) -> Void = { _ in }
        private var isFarDown = false
        private var toTop: Int
        weak var table: UITableView?
        var swiper: RowSwiper?
        var head: UIHostingController<Header>?
        /// Under the last row: a rule, and while the next page is being read, `LoadingMore`.
        let foot = UIView(frame: CGRect(x: 0, y: 0, width: 0, height: Theme.hairline))
        private let loadingFoot = UIHostingController(rootView: LoadingMore())
        private var loadingMore = false
        private var rows: [ThreadRow] = []
        private var texts: [String: (ThreadRow, RowText)] = [:]
        private var ready = true
        private var prefetch: DispatchWorkItem?
        /// Keeps the list where it was left, across the app being made anew.
        var keeper: ScrollKeeper?

        init(store: MailStore, open: @escaping (String) -> Void, toTop: Int) {
            self.store = store
            self.open = open
            self.toTop = toTop
            super.init()
            let rule = UIView(frame: CGRect(x: 0, y: 0, width: 0, height: Theme.hairline))
            rule.backgroundColor = Tokens.border.platform
            rule.autoresizingMask = [.flexibleWidth]
            foot.addSubview(rule)
            loadingFoot.view.backgroundColor = .clear
            loadingFoot.safeAreaRegions = []
            loadingFoot.view.frame = CGRect(x: 0, y: Theme.hairline, width: 0, height: LoadingMore.height)
            loadingFoot.view.autoresizingMask = [.flexibleWidth]
            loadingFoot.view.isHidden = true
            foot.addSubview(loadingFoot.view)
        }

        func update(rows: [ThreadRow], editing: Bool, checked: Set<String>, ready: Bool, headerHeight: CGFloat, loadingMore: Bool) {
            guard let table else { return }
            if loadingMore != self.loadingMore {
                self.loadingMore = loadingMore
                loadingFoot.view.isHidden = !loadingMore
                foot.frame.size.height = Theme.hairline + (loadingMore ? LoadingMore.height : 0)
                table.tableFooterView = foot
            }
            if let head, table.tableHeaderView?.frame.height != headerHeight {
                head.view.frame = CGRect(x: 0, y: 0, width: table.bounds.width, height: headerHeight)
                table.tableHeaderView = head.view
            }
            let switched = ready && !self.ready
            let changed = Self.changes(from: self.rows, to: rows)
            let removed = switched ? nil : Self.removals(from: self.rows, to: rows)
            let old = self.rows
            self.rows = rows
            self.ready = ready
            if let removed {
                table.performBatchUpdates {
                    table.deleteRows(at: removed.map { IndexPath(row: $0, section: 0) }, with: .fade)
                }
                let kept = Dictionary(old.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
                let visible = Set(table.indexPathsForVisibleRows ?? [])
                let paths = rows.indices.filter { kept[rows[$0].id] != rows[$0] }.map { IndexPath(row: $0, section: 0) }.filter(visible.contains)
                if !paths.isEmpty { table.reconfigureRows(at: paths) }
            } else if let changed {
                let visible = Set(table.indexPathsForVisibleRows ?? [])
                let paths = changed.map { IndexPath(row: $0, section: 0) }.filter(visible.contains)
                if !paths.isEmpty { table.reconfigureRows(at: paths) }
            } else {
                table.reloadData()
            }
            if switched { keeper?.top() }
            if table.isEditing != editing { table.setEditing(editing, animated: true) }
            if editing { pick(checked) }
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

        /// The rows taken out when the list is the one before less some threads, in the same
        /// order, so they leave with the rows under them closing up.
        private static func removals(from old: [ThreadRow], to new: [ThreadRow]) -> [Int]? {
            guard new.count < old.count, old.count - new.count <= 50 else { return nil }
            var removed: [Int] = []
            var next = 0
            for (index, row) in old.enumerated() {
                if next < new.count, new[next].id == row.id {
                    next += 1
                } else {
                    removed.append(index)
                }
            }
            return next == new.count ? removed : nil
        }

        /// Shows the store's selection as the table's.
        private func pick(_ checked: Set<String>) {
            guard let table else { return }
            let shown = Set((table.indexPathsForSelectedRows ?? []).compactMap { rows.indices.contains($0.row) ? rows[$0.row].id : nil })
            guard shown != checked else { return }
            for (index, row) in rows.enumerated() where checked.contains(row.id) != shown.contains(row.id) {
                let path = IndexPath(row: index, section: 0)
                if checked.contains(row.id) {
                    table.selectRow(at: path, animated: false, scrollPosition: .none)
                } else {
                    table.deselectRow(at: path, animated: false)
                }
            }
        }

        private func text(for row: ThreadRow) -> RowText {
            if let (kept, text) = texts[row.id], kept == row { return text }
            let text = RowText(row)
            texts[row.id] = (row, text)
            return text
        }

        func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }

        func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
            let cell = tableView.dequeueReusableCell(withIdentifier: ThreadCell.identifier, for: indexPath)
            let row = rows[indexPath.row]
            (cell as? ThreadCell)?.configure(row: row, text: text(for: row), first: indexPath.row == 0, swiper: swiper)
            return cell
        }

        func tableView(_ tableView: UITableView, willDisplay cell: UITableViewCell, forRowAt indexPath: IndexPath) {
            if indexPath.row > rows.count - 30 { store.loadMore() }
        }

        /// A tap while a row's commands are out only puts them away.
        func tableView(_ tableView: UITableView, shouldHighlightRowAt indexPath: IndexPath) -> Bool {
            tableView.isEditing || swiper?.isOut != true
        }

        func tableView(_ tableView: UITableView, willSelectRowAt indexPath: IndexPath) -> IndexPath? {
            guard !tableView.isEditing, let swiper, swiper.isOut else { return indexPath }
            swiper.close()
            return nil
        }

        func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
            swiper?.close()
        }

        func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
            let id = rows[indexPath.row].id
            guard !tableView.isEditing else {
                store.selection.insert(id)
                return
            }
            tableView.deselectRow(at: indexPath, animated: true)
            open(id)
        }

        func tableView(_ tableView: UITableView, didDeselectRowAt indexPath: IndexPath) {
            guard tableView.isEditing, rows.indices.contains(indexPath.row) else { return }
            store.selection.remove(rows[indexPath.row].id)
        }

        func scrollViewDidScroll(_ scrollView: UIScrollView) {
            schedulePrefetch()
            let far = scrollView.contentOffset.y + scrollView.adjustedContentInset.top > scrollView.bounds.height
            guard far != isFarDown else { return }
            isFarDown = far
            // Scrolls also come while SwiftUI updates the table, when its state can't change.
            Task { self.farDown(far) }
        }

        func goToTop(_ request: Int) {
            guard request != toTop, let table else { return }
            toTop = request
            swiper?.close()
            table.setContentOffset(CGPoint(x: table.contentOffset.x, y: -table.adjustedContentInset.top), animated: true)
        }

        @objc func pulled(_ control: UIRefreshControl) {
            Task {
                await refresh?()
                control.endRefreshing()
            }
        }

        /// Asks for the bodies of the rows on screen once the list stops moving.
        private func schedulePrefetch() {
            prefetch?.cancel()
            let work = DispatchWorkItem { [weak self] in
                MainActor.assumeIsolated {
                    guard let self, let table = self.table else { return }
                    let ids = (table.indexPathsForVisibleRows ?? []).compactMap { self.rows.indices.contains($0.row) ? self.rows[$0.row].id : nil }
                    self.store.prefetch(ids)
                }
            }
            prefetch = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: work)
        }
    }
}

/// The table, which keeps the drawer out of a swipe while a row's commands are out.
final class ThreadTable: UITableView, OwnsRightSwipe {
    weak var swiper: RowSwiper?
    var ownsRightSwipe: Bool { swiper?.isOut == true }
    private var topInset: CGFloat = 0

    /// The bar's height arrives after the table is made; a list at the top stays at the top.
    override func adjustedContentInsetDidChange() {
        super.adjustedContentInsetDidChange()
        let atTop = contentOffset.y <= -topInset + 1
        topInset = adjustedContentInset.top
        guard atTop else { return }
        contentOffset.y = -topInset
    }
}

/// One thread on three lines: who wrote and the star, the subject, the snippet and the date. A
/// swipe slides it aside over its commands.
final class ThreadCell: UITableViewCell {
    static let identifier = "thread"
    private let slider = UIView()
    private let canvas = RowCanvas()
    private let tray = SwipeTray()
    private weak var swiper: RowSwiper?
    private var first = false
    private(set) var row: ThreadRow?
    var swipe = SwipeLayout()

    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        backgroundColor = .clear
        tray.frame = contentView.bounds
        tray.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        tray.isHidden = true
        tray.tapped = { [weak self] command in
            guard let self else { return }
            self.swiper?.commit(command, on: self)
        }
        contentView.addSubview(tray)
        slider.frame = contentView.bounds
        slider.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        contentView.addSubview(slider)
        canvas.isOpaque = false
        canvas.frame = slider.bounds
        canvas.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        slider.addSubview(canvas)
        let pressed = UIView()
        pressed.backgroundColor = Tokens.backgroundAccentLargerStronger.platform
        selectedBackgroundView = pressed
    }

    required init?(coder: NSCoder) { nil }

    func configure(row: ThreadRow, text: RowText, first: Bool, swiper: RowSwiper?) {
        self.row = row
        self.first = first
        self.swiper = swiper
        canvas.row = row
        canvas.text = text
        canvas.first = first
        canvas.setNeedsDisplay()
        if !tray.isHidden { tray.show(swipe.commands, for: row, first: first) }
        accessibilityLabel = "\(row.unread ? "Unread, " : "")\(row.senders), \(row.subject), \(row.date)"
    }

    override var accessibilityCustomActions: [UIAccessibilityCustomAction]? {
        get { row.flatMap { swiper?.accessibilityActions(for: $0) } }
        set {}
    }

    override func prepareForReuse() {
        super.prepareForReuse()
        swiper?.forget(self)
        endSwipe()
    }

    /// Shows the commands behind the row; one caught as it slides is held where it is.
    func beginSwipe(_ commands: [ThreadCommand]) {
        guard let row else { return }
        if slider.layer.animationKeys() != nil, let shown = slider.layer.presentation()?.affineTransform().tx {
            swipe.pulled = -shown
        }
        Self.stopAnimations(in: contentView)
        swipe.commands = commands
        tray.show(commands, for: row, first: first)
        tray.isHidden = false
        slider.backgroundColor = Tokens.background.platform
        UIView.performWithoutAnimation { layoutSwipe() }
    }

    func layoutSwipe() {
        swipe.width = contentView.bounds.width
        slider.transform = CGAffineTransform(translationX: -swipe.offset, y: 0)
        tray.layout(swipe)
    }

    private static func stopAnimations(in view: UIView) {
        view.layer.removeAllAnimations()
        view.subviews.forEach(stopAnimations)
    }

    func endSwipe() {
        swipe.pulled = 0
        swipe.leaving = nil
        slider.transform = .identity
        slider.backgroundColor = .clear
        tray.isHidden = true
    }
}

private final class RowCanvas: UIView {
    var row: ThreadRow?
    var text: RowText?
    var first = false

    override func draw(_ rect: CGRect) {
        guard let row, let text else { return }
        if !first {
            Tokens.border.platform.setFill()
            UIRectFill(CGRect(x: 0, y: 0, width: bounds.width, height: Theme.hairline))
        }
        Theme.accountColor(row.color).platform.setFill()
        let top = first ? 0 : Theme.hairline
        UIRectFill(CGRect(x: 0, y: top, width: Theme.iosAccountBarWidth, height: bounds.height - top))
        let left: CGFloat = 16
        let right = bounds.width - 16
        let star = CGRect(x: right - 18, y: 14, width: 18, height: 18)
        var x = left
        if row.unread {
            Tokens.primary.platform.setFill()
            UIBezierPath(ovalIn: CGRect(x: left, y: 20, width: 8, height: 8)).fill()
            x += 15
        }
        draw(text.senders, x: x, y: 12, width: star.minX - 10 - x)
        let starColor = row.starred ? Tokens.star.platform : Tokens.mutedStrongestForeground.platform
        UIImage.symbol(row.starred ? .starFilled : .star, side: star.width).withTintColor(starColor, renderingMode: .alwaysOriginal).draw(in: star)
        var subjectWidth = right - left
        if row.attachment {
            let clip = CGRect(x: right - 15, y: 38, width: 15, height: 15)
            UIImage.symbol(.paperclip, side: 15).withTintColor(Tokens.mutedStrongerForeground.platform, renderingMode: .alwaysOriginal).draw(in: clip)
            subjectWidth -= 22
        }
        draw(text.subject, x: left, y: 36, width: subjectWidth)
        let dateWidth = ceil(text.date.size().width)
        draw(text.date, x: right - dateWidth, y: 60, width: dateWidth + 1)
        draw(text.snippet, x: left, y: 59, width: right - dateWidth - 10 - left)
    }

    private func draw(_ text: NSAttributedString, x: CGFloat, y: CGFloat, width: CGFloat) {
        guard width > 8 else { return }
        text.draw(with: CGRect(x: x, y: y, width: width, height: 24), options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine], context: nil)
    }
}
#endif
