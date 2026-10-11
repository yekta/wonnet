import SwiftUI

/// The arrow back to the top, near the head of a list scrolled past its first page.
struct ToTop: View {
    let shown: Bool
    let action: () -> Void

    var body: some View {
        ZStack {
            if shown {
                FloatingButton(symbol: .arrowUp, help: "Back to Top", small: true, action: action)
                    .padding(.top, Space.m)
                    .transition(.opacity.combined(with: .scale(scale: 0.8)))
            }
        }
        .animation(.easeOut(duration: 0.15), value: shown)
    }
}
