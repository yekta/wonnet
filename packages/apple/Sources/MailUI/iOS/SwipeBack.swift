#if os(iOS)
import SwiftUI
import UIKit

/// Keeps the swipe back, from the screen's edge and from anywhere on a page, which UIKit turns off
/// while the navigation bar is hidden. Every page draws a bar of its own, so the system's stays
/// hidden: shown on one page, it slid out with that page on a swipe back.
struct SwipeBack: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> Installer { Installer() }
    func updateUIViewController(_ installer: Installer, context: Context) {}

    final class Installer: UIViewController {
        private var gates: [Gate] = []

        override func viewDidAppear(_ animated: Bool) {
            super.viewDidAppear(animated)
            guard let navigation = navigationController else { return }
            var recognizers = [navigation.interactivePopGestureRecognizer]
            if #available(iOS 26, *) { recognizers.append(navigation.interactiveContentPopGestureRecognizer) }
            for case let recognizer? in recognizers where !(recognizer.delegate is Gate) {
                let gate = Gate(navigation: navigation, system: recognizer.delegate)
                gates.append(gate)
                recognizer.delegate = gate
            }
        }
    }

    /// Lets a swipe back begin whenever there is a page to go back to and no push or pop is under
    /// way; asks the system's delegate the rest of what the protocol has.
    final class Gate: NSObject, UIGestureRecognizerDelegate {
        private weak var navigation: UINavigationController?
        private weak var system: UIGestureRecognizerDelegate?

        init(navigation: UINavigationController, system: UIGestureRecognizerDelegate?) {
            self.navigation = navigation
            self.system = system
        }

        func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
            guard let navigation, navigation.viewControllers.count > 1 else { return false }
            return navigation.transitionCoordinator == nil
        }

        override func responds(to selector: Selector!) -> Bool {
            super.responds(to: selector) || (Self.isPublic(selector) && system?.responds(to: selector) == true)
        }

        override func forwardingTarget(for selector: Selector!) -> Any? {
            Self.isPublic(selector) ? system : nil
        }

        /// Whether the protocol has it. The system's private questions go unasked: one of them
        /// turns every touch away while the bar is hidden.
        private static func isPublic(_ selector: Selector) -> Bool {
            protocol_getMethodDescription(UIGestureRecognizerDelegate.self, selector, false, true).name != nil
        }
    }
}
#endif
