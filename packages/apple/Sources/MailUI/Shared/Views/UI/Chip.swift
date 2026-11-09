import SwiftUI

/// A small pill: an address in a field, a file, a thread's label. `invalid` is an address that
/// can't be sent to; `success` and `warning` are a state in its colour, as an application that is
/// verified or not, on a smaller pill that sits beside a name. `removeHelp` is what the x says it does, "Remove" and the title without it.
struct Chip: View {
    enum Variant {
        case plain, invalid, success, warning
    }

    let title: String
    var symbol: Symbol?
    var detail: String?
    var variant: Variant = .plain
    var pending = false
    var help: String?
    var removeHelp: String?
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
        HStack(spacing: badge ? Space.xxs : Space.xs) {
            ButtonIcon(symbol: symbol, pending: pending, size: badge ? 11 : 12, tint: tone?.color ?? Tokens.mutedForeground.color)
            Text(title).font(.ui(.caption)).lineLimit(1).truncationMode(.middle)
            if let detail {
                Text(detail).textStyle(.footnote).lineLimit(1)
            }
            if let remove {
                Button(action: remove) { RemoveFace(fill: onCard ? Theme.cardAccentStronger : Tokens.backgroundSecondaryAccentStronger) }
                    .buttonStyle(.press)
                    .help(removeHelp ?? "Remove \(title)")
                    .accessibilityLabel(removeHelp ?? "Remove \(title)")
                    .padding(.trailing, -(Space.s + 2))
            }
        }
        // A badge's words end a little further from its edge than its icon starts, so the round
        // end doesn't crowd them.
        .padding(.leading, badge ? Space.xs : Space.s + 2)
        .padding(.trailing, badge ? Space.s : Space.s + 2)
        .frame(height: (badge ? Theme.badgeHeight : Theme.chipHeight) * Platform.scale)
        .foregroundStyle(tone?.color ?? Tokens.foreground.color)
        .background(Capsule().fill(fill.color))
        .overlay(Capsule().strokeBorder(variant == .invalid ? Tokens.destructive.opacity(Opacity.colorBorder).color : .clear, lineWidth: Theme.hairline))
        .contentShape(Capsule())
    }

    /// A state beside a name, smaller than a chip that is a thing of its own.
    private var badge: Bool { variant == .success || variant == .warning }

    /// The colour of a chip that says something with it.
    private var tone: ThemeColor? {
        switch variant {
        case .plain: nil
        case .invalid: Tokens.destructive
        case .success: Tokens.success
        case .warning: Tokens.warning
        }
    }

    private var fill: ThemeColor {
        if variant == .success || variant == .warning, let tone { return tone.opacity(Opacity.colorTint) }
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
