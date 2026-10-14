#if os(iOS)
import SwiftUI
import UIKit

/// A swipe to the left on a thread row. One to three commands show behind it, from the screen's
/// edge inwards, and the row rests aside on them to tap; a longer swipe does the first, at the
/// edge, a lone one sooner. What takes the thread out of the list slides its row away.
@MainActor
final class RowSwiper: NSObject, UIGestureRecognizerDelegate {
    let pan = UIPanGestureRecognizer()
    private weak var table: UITableView?
    private let store: MailStore
    /// The row moved or resting aside.
    private weak var cell: ThreadCell?
    private var offsetAtStart: CGFloat = 0
    private let arming = UIImpactFeedbackGenerator(style: .medium)
    private let disarming = UIImpactFeedbackGenerator(style: .light)

    init(table: UITableView, store: MailStore) {
        self.table = table
        self.store = store
        super.init()
        pan.addTarget(self, action: #selector(panned))
        pan.delegate = self
        table.addGestureRecognizer(pan)
        table.panGestureRecognizer.require(toFail: pan)
    }

    var isOut: Bool { cell != nil }

    func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
        guard let table, !table.isEditing else { return false }
        let speed = pan.velocity(in: table)
        guard abs(speed.x) > abs(speed.y) * 1.3 else { return false }
        guard let touched = touchedCell(), touched.row != nil else {
            close()
            return false
        }
        if let cell, cell !== touched { close() }
        return cell != nil || speed.x < 0
    }

    private func touchedCell() -> ThreadCell? {
        guard let table, let path = table.indexPathForRow(at: pan.location(in: table)) else { return nil }
        return table.cellForRow(at: path) as? ThreadCell
    }

    @objc private func panned() {
        switch pan.state {
        case .began:
            guard let touched = touchedCell(), let row = touched.row else { return }
            cell = touched
            touched.beginSwipe(commands(for: row))
            offsetAtStart = touched.swipe.pulled
            arming.prepare()
        case .changed:
            guard let cell else { return }
            follow(cell, to: offsetAtStart - pan.translation(in: table).x)
        case .ended, .cancelled, .failed:
            guard let cell else { return }
            release(cell, speed: -pan.velocity(in: table).x)
        default:
            break
        }
    }

    /// What a swipe shows on a row: a draft's is only deleting it.
    private func commands(for row: ThreadRow) -> [ThreadCommand] {
        row.draftId == nil ? store.swipeCommands : [.trash]
    }

    /// The commands as VoiceOver offers them on a row.
    func accessibilityActions(for row: ThreadRow) -> [UIAccessibilityCustomAction] {
        commands(for: row).map { command in
            UIAccessibilityCustomAction(name: command.title(store, [row.id])) { [weak self] _ in
                self?.perform(command, row.id)
                return true
            }
        }
    }

    private func perform(_ command: ThreadCommand, _ id: String) {
        store.run(command, on: [id])
    }

    private func follow(_ cell: ThreadCell, to pulled: CGFloat) {
        let wasArmed = cell.swipe.armed
        cell.swipe.pulled = max(0, pulled)
        guard cell.swipe.armed != wasArmed else { return cell.layoutSwipe() }
        cell.swipe.armed ? arming.impactOccurred() : disarming.impactOccurred()
        UIView.animate(springDuration: 0.3, bounce: 0.25, options: [.allowUserInteraction, .beginFromCurrentState]) {
            cell.layoutSwipe()
        }
    }

    private func release(_ cell: ThreadCell, speed: CGFloat) {
        let swipe = cell.swipe
        guard let edge = swipe.commands.first else { return close() }
        guard !swipe.armed else { return commit(edge, on: cell) }
        let opens = abs(speed) > 400 ? speed > 0 : swipe.offset > swipe.rest / 2
        guard opens else { return close(speed: speed) }
        settle(cell, to: swipe.rest, speed: speed)
    }

    /// Does the command, then puts the row back or, when the thread leaves the list, slides it away.
    func commit(_ command: ThreadCommand, on cell: ThreadCell) {
        guard let id = cell.row?.id else { return close() }
        guard command.takesRowAway else {
            perform(command, id)
            return close()
        }
        self.cell = nil
        cell.swipe.leaving = command
        UIView.animate(withDuration: 0.2, delay: 0, options: [.curveEaseIn, .beginFromCurrentState]) {
            cell.layoutSwipe()
        } completion: { _ in
            self.perform(command, id)
            // A list that keeps the thread, like All Mail's, gets its row back.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
                guard cell.row?.id == id, cell.swipe.leaving != nil else { return }
                cell.swipe.leaving = nil
                self.settle(cell, to: 0)
            }
        }
    }

    func close(speed: CGFloat = 0) {
        guard let cell else { return }
        self.cell = nil
        settle(cell, to: 0, speed: speed)
    }

    /// Forgets a row that is shown again for another thread.
    func forget(_ reused: ThreadCell) {
        guard cell === reused else { return }
        cell = nil
    }

    private func settle(_ cell: ThreadCell, to offset: CGFloat, speed: CGFloat = 0) {
        let distance = offset - cell.swipe.offset
        cell.swipe.pulled = offset
        let velocity = abs(distance) < 1 ? 0 : min(30, max(-30, speed / distance))
        UIView.animate(springDuration: 0.4, bounce: 0.05, initialSpringVelocity: velocity, options: [.allowUserInteraction, .beginFromCurrentState]) {
            cell.layoutSwipe()
        } completion: { _ in
            guard cell.swipe.offset == 0 else { return }
            cell.endSwipe()
        }
    }
}

/// Where a row swiped to the left is, and what it shows behind it.
struct SwipeLayout {
    /// The width each command takes behind the row.
    static let slot: CGFloat = 64
    /// The space after the last command, to the screen's edge.
    static let inset: CGFloat = 8
    static let circle: CGFloat = 40
    static let glyph: CGFloat = 19

    var commands: [ThreadCommand] = []
    var width: CGFloat = 0
    /// How far the finger has pulled the row, or where it was put.
    var pulled: CGFloat = 0
    /// The command the row slides off the screen with.
    var leaving: ThreadCommand?

    /// Where the commands rest.
    var rest: CGFloat { CGFloat(commands.count) * Self.slot + Self.inset }
    /// From here on, letting go does the first command: far enough past where the commands rest
    /// that opening the row doesn't do it, and for two or three never under 45% of the row.
    var longSwipe: CGFloat {
        guard commands.count > 1 else { return rest + 40 }
        return max(rest + 40, width * 0.45)
    }

    var armed: Bool {
        guard leaving == nil else { return true }
        return pulled >= longSwipe
    }

    /// How far the row is aside.
    var offset: CGFloat {
        guard leaving == nil else { return width }
        return min(pulled, width)
    }
}

/// The commands behind a row: a circle each, in a wash of its colour, filled with it once
/// letting go would do it.
final class SwipeTray: UIView {
    private var buttons: [SwipeButton] = []
    var tapped: ((ThreadCommand) -> Void)?

    func show(_ commands: [ThreadCommand], for row: ThreadRow) {
        while buttons.count < commands.count {
            let button = SwipeButton()
            button.addTarget(self, action: #selector(pressed(_:)), for: .touchUpInside)
            addSubview(button)
            buttons.append(button)
        }
        for (index, button) in buttons.enumerated() {
            button.isHidden = index >= commands.count
            guard index < commands.count else { continue }
            button.show(commands[index], for: row)
        }
    }

    @objc private func pressed(_ button: SwipeButton) {
        guard let command = button.command else { return }
        tapped?(command)
    }

    func layout(_ swipe: SwipeLayout) {
        let edge = bounds.width - swipe.offset
        let count = swipe.commands.count
        // The row uncovers the commands where they rest, the edge's first; past them it pulls them along.
        let pulledAlong = max(0, swipe.offset - swipe.rest)
        let doing = swipe.leaving ?? swipe.commands.first
        for (index, button) in buttons.prefix(count).enumerated() {
            guard swipe.armed else {
                let fromEdge = SwipeLayout.inset + CGFloat(index) * SwipeLayout.slot
                let uncovered = min(1, max(0, (swipe.offset - fromEdge) / SwipeLayout.slot))
                let center = bounds.width - fromEdge - SwipeLayout.slot / 2 - pulledAlong
                place(button, at: center, scale: 0.7 + 0.3 * uncovered, alpha: uncovered, armed: false)
                continue
            }
            let done = swipe.commands[index] == doing
            place(button, at: edge + SwipeLayout.slot / 2, scale: done ? 1 : 0.6, alpha: done ? 1 : 0, armed: done)
        }
    }

    private func place(_ button: SwipeButton, at center: CGFloat, scale: CGFloat, alpha: CGFloat, armed: Bool) {
        button.bounds = CGRect(x: 0, y: 0, width: SwipeLayout.slot, height: bounds.height)
        button.center = CGPoint(x: center, y: bounds.midY)
        button.face.transform = CGAffineTransform(scaleX: scale, y: scale)
        button.alpha = alpha
        button.armed = armed
    }
}

final class SwipeButton: UIControl {
    let face = UIView()
    private let glyph = UIImageView()
    private(set) var command: ThreadCommand?

    override init(frame: CGRect) {
        super.init(frame: frame)
        face.isUserInteractionEnabled = false
        face.bounds = CGRect(x: 0, y: 0, width: SwipeLayout.circle, height: SwipeLayout.circle)
        face.layer.cornerRadius = SwipeLayout.circle / 2
        face.layer.cornerCurve = .circular
        glyph.contentMode = .center
        glyph.frame = face.bounds
        face.addSubview(glyph)
        addSubview(face)
    }

    required init?(coder: NSCoder) { nil }

    func show(_ command: ThreadCommand, for row: ThreadRow) {
        self.command = command
        glyph.image = UIImage.symbol(command.swipeSymbol(row), side: SwipeLayout.glyph).withRenderingMode(.alwaysTemplate)
        accessibilityLabel = command.swipeName
        paint()
    }

    var armed = false {
        didSet {
            guard armed != oldValue else { return }
            paint()
        }
    }

    private func paint() {
        guard let tint = command?.swipeTint else { return }
        face.backgroundColor = armed ? tint.fill.platform : tint.fill.opacity(Opacity.colorTintStronger).platform
        glyph.tintColor = armed ? tint.glyph.platform : tint.fill.platform
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        face.center = CGPoint(x: bounds.midX, y: bounds.midY)
    }
}
#endif
