import SwiftUI

/// An icon on a circle over a page: Compose over the list on iOS, and smaller, the way back to
/// the top of a list. The page's colour with its border and a little shadow, so it stands off
/// the list scrolling under it.
struct FloatingButton: View {
    static let side: CGFloat = 56
    static let smallSide: CGFloat = 40

    let symbol: Symbol
    let help: String
    var small = false
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(symbol: symbol, small: small) }
            .buttonStyle(.press)
            .accessibilityLabel(help)
            .help(help)
    }

    private struct Face: View {
        let symbol: Symbol
        let small: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            let side = small ? FloatingButton.smallSide : FloatingButton.side
            Image(symbol, size: small ? ControlSize.large.icon : 22)
                .foregroundStyle(Tokens.foreground.color)
                .frame(width: side, height: side)
                .background(
                    Circle()
                        .fill(hovered ? Tokens.backgroundAccent.color : Tokens.background.color)
                        .shadow(.sm)
                )
                .overlay(Circle().strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
        }
    }
}
