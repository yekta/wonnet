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
#endif
