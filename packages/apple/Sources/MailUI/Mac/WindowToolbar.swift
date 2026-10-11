#if os(macOS)
import AppKit
import SwiftUI

extension View {
    /// The window's toolbar is only the bar the window's buttons sit in, with our top bar drawn
    /// under it: no background, no title.
    func windowToolbarUnderTopBar() -> some View {
        modifier(WindowToolbarUnderTopBar())
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

private struct WindowToolbarUnderTopBar: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content
                .toolbarBackground(.hidden, for: .windowToolbar)
                .toolbar(removing: .title)
        } else {
            content
                .toolbarBackground(.hidden, for: .windowToolbar)
                .background(WindowTitleHider())
        }
    }
}

private struct WindowTitleHider: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async { view.window?.titleVisibility = .hidden }
    }
}
#endif
