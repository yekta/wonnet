#if os(macOS)
import AppKit
import SwiftUI

/// A page of rows that AppKit scrolls, for one too long for a SwiftUI `ScrollView`, which lays
/// out, hit-tests and draws again every row in sight on each step of a scroll. Each row is a
/// hosting view of its own, in the window only while near sight, where it says how tall it is.
/// A row is drawn again only when its `version` changes; what it reads from the store it follows
/// itself.
struct ScrollStack: NSViewRepresentable {
    struct Row {
        let id: String
        var version: AnyHashable = 0
        let content: AnyView
    }

    let rows: [Row]
    /// The row to bring to the top, let go once it is there.
    @Binding var target: String?
    @Environment(MailStore.self) private var store

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        let stack = RowStack()
        stack.autoresizingMask = [.width]
        scroll.documentView = stack
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.automaticallyAdjustsContentInsets = false
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let stack = scroll.documentView as? RowStack else { return }
        stack.show(rows.map { row in (row.id, row.version, AnyView(row.content.environment(store))) })
        guard let target else { return }
        stack.scroll(to: target)
        DispatchQueue.main.async { self.target = nil }
    }

    /// It takes the room it is offered: asked what fits, AppKit would lay out every row.
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSScrollView, context: Context) -> CGSize? {
        proposal.replacingUnspecifiedDimensions()
    }
}

/// The rows one under another, as tall as they are together.
final class RowStack: NSView {
    private final class Row {
        let id: String
        var version: AnyHashable
        let host: NSHostingController<AnyView>
        var frame = CGRect.zero
        /// How tall it said it is; until it has, it is as tall as most rows.
        var height: CGFloat?
        weak var stack: RowStack?

        init(id: String, version: AnyHashable, content: AnyView) {
            self.id = id
            self.version = version
            host = NSHostingController(rootView: AnyView(EmptyView()))
            host.sizingOptions = []
            // Scrolled under the toolbar, a row would otherwise be laid out again on every step.
            host.safeAreaRegions = []
            host.view.wantsLayer = true
            show(content)
        }

        /// The content at its own height, which it says once drawn and whenever it changes: at
        /// another width, or with a box opening in it.
        func show(_ content: AnyView) {
            host.rootView = AnyView(
                content
                    .fixedSize(horizontal: false, vertical: true)
                    .onGeometryChange(for: CGFloat.self, of: \.size.height) { [weak self] height in self?.grew(to: height) }
            )
        }

        /// Measured in place, for a row in sight before it has said how tall it is.
        func measure(width: CGFloat) {
            height = host.sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude)).height
        }

        private func grew(to height: CGFloat) {
            guard abs(height - frame.height) > 0.5 || self.height == nil else { return }
            self.height = height
            stack?.resized(self)
        }
    }

    /// How long after the last step a scroll is over.
    private static let settle: TimeInterval = 0.15
    /// How far past sight rows are in the window, in heights of the visible rect, so that they
    /// are drawn before they are seen.
    private static let reach: CGFloat = 0.5
    private var rows: [Row] = []
    /// What a row that hasn't said how tall it is is taken to be: the middle of those that have.
    private var estimate: CGFloat {
        let heights = rows.compactMap(\.height).sorted()
        return heights.isEmpty ? 0 : heights[heights.count / 2]
    }
    /// The row to bring to the top once the rows are laid out.
    private var target: String?
    /// How far the page moves at the next layout, for rows above sight that changed height.
    private var shift: CGFloat = 0
    /// While it scrolls, the pointer is the stack's, so the row under it isn't asked on every
    /// step what the pointer is over.
    private var scrolling = false

    override var isFlipped: Bool { true }

    /// The window isn't dragged by the rows, so it doesn't ask every row on every step where it
    /// may be.
    override var mouseDownCanMoveWindow: Bool { false }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        NotificationCenter.default.removeObserver(self)
        guard let clip = superview as? NSClipView else { return }
        clip.postsBoundsChangedNotifications = true
        clip.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(self, selector: #selector(scrolled), name: NSView.boundsDidChangeNotification, object: clip)
        NotificationCenter.default.addObserver(self, selector: #selector(scrolled), name: NSView.frameDidChangeNotification, object: clip)
    }

    /// The rows as they are now, keeping the hosting view of each that was there before.
    func show(_ list: [(id: String, version: AnyHashable, content: AnyView)]) {
        var kept = Dictionary(uniqueKeysWithValues: rows.map { ($0.id, $0) })
        rows = list.map { id, version, content in
            guard let row = kept.removeValue(forKey: id) else {
                let row = Row(id: id, version: version, content: content)
                row.stack = self
                return row
            }
            if row.version != version {
                row.version = version
                row.show(content)
            }
            return row
        }
        kept.values.forEach { $0.host.view.removeFromSuperview() }
        needsLayout = true
    }

    func scroll(to id: String) {
        target = id
        needsLayout = true
    }

    override func layout() {
        super.layout()
        arrange()
        if shift != 0, let clip = enclosingScrollView?.contentView {
            clip.setBoundsOrigin(NSPoint(x: 0, y: clip.bounds.minY + shift))
            shift = 0
        }
        guard let id = target, let row = rows.first(where: { $0.id == id }), let clip = enclosingScrollView?.contentView else { return }
        target = nil
        // The rows around it are measured once in sight, which can move it: go again until it stays.
        for _ in 0..<3 {
            let top = min(row.frame.minY, max(frame.height - clip.bounds.height, 0))
            guard clip.bounds.minY != top else { return }
            clip.setBoundsOrigin(NSPoint(x: 0, y: top))
            arrange()
        }
    }

    /// Lays the rows one under another, measuring first those in sight that haven't said how
    /// tall they are.
    private func arrange() {
        let width = bounds.width
        guard width > 0 else { return }
        let sight = visibleRect
        var top: CGFloat = 0
        var estimate = self.estimate
        for row in rows {
            row.frame = CGRect(x: 0, y: top, width: width, height: row.height ?? estimate)
            if row.height == nil, row.frame.intersects(sight) || row.frame.height == 0 {
                row.measure(width: width)
                row.frame.size.height = row.height ?? 0
                estimate = self.estimate
            }
            top += row.frame.height
        }
        if frame.height != top { setFrameSize(NSSize(width: width, height: top)) }
        placeRows()
    }

    /// A row said it is taller or shorter. One above sight moves the page by as much, so what is
    /// in sight stays put.
    private func resized(_ row: Row) {
        if row.frame.minY < visibleRect.minY { shift += (row.height ?? 0) - row.frame.height }
        needsLayout = true
    }

    @objc private func scrolled() {
        arrange()
        scrolling = true
        NSObject.cancelPreviousPerformRequests(withTarget: self, selector: #selector(settled), object: nil)
        perform(#selector(settled), with: nil, afterDelay: Self.settle)
    }

    @objc private func settled() {
        scrolling = false
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard scrolling else { return super.hitTest(point) }
        guard frame.contains(point) else { return nil }
        return self
    }

    /// Puts the rows near sight in the window, at their place, and takes the others out.
    private func placeRows() {
        let shown = visibleRect.insetBy(dx: 0, dy: -visibleRect.height * Self.reach)
        for row in rows {
            let view = row.host.view
            guard row.frame.intersects(shown) else {
                if view.superview != nil { view.removeFromSuperview() }
                continue
            }
            if view.frame != row.frame { view.frame = row.frame }
            if view.superview == nil { addSubview(view) }
        }
    }
}
#endif
