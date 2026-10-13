import SwiftUI

/// A bar floating over a page, on the system's glass: icon buttons spread evenly across a
/// capsule, as the commands under an open thread. Before iOS 26 it is the page's colour on a
/// bordered capsule with a little shadow, as `FloatingButton`.
struct GlassBar<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        Spread { content }
            .padding(.horizontal, Space.xs)
            .frame(maxWidth: .infinity)
            .frame(height: Theme.barHeight)
            .glassFace(interactive: false, hovered: false)
    }

    /// Shares its width equally among its children, each centred in its share.
    private struct Spread: Layout {
        func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
            let tallest = subviews.map { $0.sizeThatFits(.unspecified).height }.max() ?? 0
            return CGSize(width: proposal.width ?? 0, height: proposal.height ?? tallest)
        }

        func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
            guard !subviews.isEmpty else { return }
            let share = bounds.width / CGFloat(subviews.count)
            for (index, subview) in subviews.enumerated() {
                let center = CGPoint(x: bounds.minX + share * (CGFloat(index) + 0.5), y: bounds.midY)
                subview.place(at: center, anchor: .center, proposal: .unspecified)
            }
        }
    }
}

/// One action on a circle of the same glass, beside a `GlassBar`: Reply beside a thread's
/// commands.
struct GlassCircle: View {
    let symbol: Symbol
    let help: String
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(symbol: symbol) }
            .buttonStyle(.press)
            .help(help)
            .accessibilityLabel(help)
    }

    private struct Face: View {
        let symbol: Symbol
        @Environment(\.hovered) private var hovered

        var body: some View {
            Image(symbol, size: ControlSize.large.icon)
                .foregroundStyle(Tokens.foreground.color)
                .frame(width: Theme.barHeight, height: Theme.barHeight)
                .glassFace(interactive: true, hovered: hovered)
                .contentShape(Circle())
        }
    }
}

private struct GlassFace: ViewModifier {
    let interactive: Bool
    let hovered: Bool

    func body(content: Content) -> some View {
        if #available(iOS 26, macOS 26, *) {
            content.glassEffect(interactive ? .regular.interactive() : .regular, in: .capsule)
        } else {
            content
                .background(Capsule().fill(hovered ? Tokens.backgroundAccent.color : Tokens.background.color).shadow(.sm))
                .overlay(Capsule().strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
        }
    }
}

private extension View {
    /// The capsule of glass, or its stand-in before iOS 26; `interactive` lets the glass answer
    /// a press.
    func glassFace(interactive: Bool, hovered: Bool) -> some View {
        modifier(GlassFace(interactive: interactive, hovered: hovered))
    }
}
