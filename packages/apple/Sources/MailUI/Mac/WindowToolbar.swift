#if os(macOS)
import AppKit
import SwiftUI

extension View {
    /// The window's toolbar is only the bar the window's buttons sit in, with our top bar drawn
    /// under it.
    func windowToolbarUnderTopBar() -> some View {
        toolbarBackground(.hidden, for: .windowToolbar)
    }
}

extension ToolbarContent {
    /// Our own button in the window's toolbar, without the glass the system puts behind one.
    @ToolbarContentBuilder
    func withoutSystemGlass() -> some ToolbarContent {
        if #available(macOS 26.0, *) {
            sharedBackgroundVisibility(.hidden)
        } else {
            self
        }
    }
}

/// Behind a button in the window's toolbar: tells where the button ends, in points from the
/// window's left edge, so the top bar under the toolbar can line its controls up with it. Measured
/// again when full screen hides the window's buttons and moves it.
struct ToolbarEdge: NSViewRepresentable {
    let onChange: (CGFloat) -> Void

    func makeNSView(context: Context) -> EdgeView { EdgeView(onChange: onChange) }

    func updateNSView(_ view: EdgeView, context: Context) {
        view.onChange = onChange
        view.report()
    }

    final class EdgeView: NSView {
        var onChange: (CGFloat) -> Void
        private var end: CGFloat?
        private var observers: [NSObjectProtocol] = []

        init(onChange: @escaping (CGFloat) -> Void) {
            self.onChange = onChange
            super.init(frame: .zero)
        }

        @available(*, unavailable) required init?(coder: NSCoder) { nil }

        deinit { observers.forEach(NotificationCenter.default.removeObserver) }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            observers.forEach(NotificationCenter.default.removeObserver)
            observers = []
            guard let window else { return }
            let names = [NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification]
            observers = names.map { name in
                NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated { self?.report() }
                }
            }
            report()
        }

        override func layout() {
            super.layout()
            report()
        }

        func report() {
            guard window != nil else { return }
            let end = convert(bounds, to: nil).maxX
            guard end != self.end else { return }
            self.end = end
            // After the layout pass that found it, so SwiftUI isn't told mid-update.
            Task { @MainActor in self.onChange(end) }
        }
    }
}
#endif
