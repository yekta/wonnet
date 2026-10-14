#if os(iOS)
import SwiftUI
import UIKit

/// A swipe across an open thread: to the left it turns to the thread under it in the list, to the
/// right to the one over it, as Newton's did. A swipe from the screen's edge stays the way back,
/// and so does one to the right with no thread over this one.
struct ThreadSwipe: UIGestureRecognizerRepresentable {
    /// Whether there is a thread that way: 1 under this one, -1 over it.
    let canTurn: (Int) -> Bool
    let moved: (CGFloat) -> Void
    let released: (_ distance: CGFloat, _ speed: CGFloat) -> Void

    func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator { Coordinator() }

    func makeUIGestureRecognizer(context: Context) -> UIPanGestureRecognizer {
        let pan = UIPanGestureRecognizer()
        pan.delegate = context.coordinator
        return pan
    }

    func updateUIGestureRecognizer(_ pan: UIPanGestureRecognizer, context: Context) {
        context.coordinator.canTurn = canTurn
    }

    func handleUIGestureRecognizerAction(_ pan: UIPanGestureRecognizer, context: Context) {
        let distance = pan.translation(in: pan.view).x
        switch pan.state {
        case .changed: moved(distance)
        case .ended: released(distance, pan.velocity(in: pan.view).x)
        case .cancelled, .failed: released(0, 0)
        default: break
        }
    }

    @MainActor final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        /// How far in from the screen's edge a swipe is the way back.
        private static let edge: CGFloat = 24
        var canTurn: (Int) -> Bool = { _ in false }

        func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
            guard let pan = recognizer as? UIPanGestureRecognizer, let window = pan.view?.window else { return false }
            let speed = pan.velocity(in: window)
            guard abs(speed.x) > abs(speed.y) * 1.3 else { return false }
            let start = pan.location(in: window).x - pan.translation(in: window).x
            guard start > Self.edge else { return false }
            return canTurn(speed.x < 0 ? 1 : -1)
        }

        /// The swipe back from the edge goes first.
        func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldRequireFailureOf other: UIGestureRecognizer) -> Bool {
            other === navigation(of: recognizer)?.interactivePopGestureRecognizer
        }

        /// The thread's scrolling and the swipe back from anywhere on the page wait to hear that
        /// this swipe isn't a turn, which they do the moment it sets out up or down.
        func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldBeRequiredToFailBy other: UIGestureRecognizer) -> Bool {
            if other is UIPanGestureRecognizer, other.view is UIScrollView { return true }
            guard #available(iOS 26, *) else { return false }
            return other === navigation(of: recognizer)?.interactiveContentPopGestureRecognizer
        }

        private func navigation(of recognizer: UIGestureRecognizer) -> UINavigationController? {
            guard let view = recognizer.view else { return nil }
            return sequence(first: view as UIResponder, next: \.next).lazy.compactMap { $0 as? UINavigationController }.first
        }
    }
}
#endif
