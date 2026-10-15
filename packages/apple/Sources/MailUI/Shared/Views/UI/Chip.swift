import SwiftUI

/// A small pill: an address in a field, a file, a thread's label. `invalid` is an address that
/// can't be sent to.
struct Chip: View {
    enum Variant {
        case plain, invalid
    }

    let title: String
    var symbol: Symbol?
    var detail: String?
    var variant: Variant = .plain
    var pending = false
    var help: String?
    var remove: (() -> Void)?
    var action: (() -> Void)?
    @Environment(\.hovered) private var hovered
    @Environment(\.onCard) private var onCard

    var body: some View {
        if let action, remove == nil {
            Button(action: action) { content }
                .buttonStyle(.press)
                .help(help ?? title)
        } else {
            content.help(help ?? title)
        }
    }

    private var content: some View {
        HStack(spacing: Space.xs) {
            ButtonIcon(symbol: symbol, pending: pending, size: 12, tint: variant == .invalid ? Tokens.destructive.color : Tokens.mutedForeground.color)
            Text(title).font(.ui(.caption)).lineLimit(1).truncationMode(.middle)
            if let detail {
                Text(detail).textStyle(.footnote).lineLimit(1)
            }
            if let remove {
                Button(action: remove) { RemoveFace(fill: onCard ? Theme.cardAccentStronger : Tokens.backgroundSecondaryAccentStronger) }
                    .buttonStyle(.press)
                    .help("Remove \(title)")
                    .accessibilityLabel("Remove \(title)")
                    .padding(.trailing, -(Space.s + 2))
            }
        }
        .padding(.horizontal, Space.s + 2)
        .frame(height: Theme.chipHeight * Platform.scale)
        .foregroundStyle(variant == .invalid ? Tokens.destructive.color : Tokens.foreground.color)
        .background(Capsule().fill(fill.color))
        .overlay(Capsule().strokeBorder(variant == .invalid ? Tokens.destructive.opacity(Opacity.colorBorder).color : .clear, lineWidth: Theme.hairline))
        .contentShape(Capsule())
    }

    private var fill: ThemeColor {
        let hovering = hovered && action != nil
        if onCard { return hovering ? Theme.cardAccentStronger : Theme.cardAccent }
        return hovering ? Tokens.backgroundSecondaryAccent : Tokens.backgroundSecondary
    }

    /// The x, filling the pill's round end: it hovers in a circle inside it, as far from the
    /// pill's top, bottom and side.
    private struct RemoveFace: View {
        let fill: ThemeColor
        @Environment(\.hovered) private var hovered

        var body: some View {
            Image(.x, size: 11)
                .foregroundStyle(hovered ? Tokens.foreground.color : Tokens.mutedStrongerForeground.color)
                .frame(width: side, height: side)
                .background(Circle().fill(hovered ? fill.color : .clear).padding(Space.xxs))
                .contentShape(Rectangle())
        }

        private var side: CGFloat { Theme.chipHeight * Platform.scale }
    }
}
