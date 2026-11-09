import SwiftUI

/// A condition of a filter as a chip: its kind's word muted, its value, and an x that takes it
/// away. Clicked, it opens what changes it; `open` while that is open. `empty` is a condition no
/// mail matches, in the warning colour with an alert before it. The chip hovers as a whole.
struct ConditionChip: View {
    let label: String
    let text: String
    var help: String?
    var empty = false
    var open = false
    let action: () -> Void
    let remove: () -> Void
    @State private var hovered = false

    var body: some View {
        HStack(spacing: 0) {
            Button(action: action) {
                HStack(spacing: Space.xxs) {
                    if empty {
                        Image(.circleAlert, size: 12).foregroundStyle(Tokens.warning.color)
                    }
                    if !label.isEmpty {
                        Text(label).foregroundStyle((empty ? Tokens.warning : Tokens.mutedForeground).color)
                    }
                    Text(text).foregroundStyle((empty ? Tokens.warning : Tokens.foreground).color)
                }
                .font(.ui(.caption))
                .lineLimit(1)
                .padding(.leading, Space.s + 2)
                .frame(height: height)
                .contentShape(Rectangle())
            }
            .buttonStyle(.press)
            // No tooltip while its menu is open: it would sit over the menu.
            .help(open ? "" : help ?? text)
            Button(action: remove) { RemoveFace(empty: empty) }
                .buttonStyle(.press)
                .help("Remove \(label) \(text)")
                .accessibilityLabel("Remove \(label) \(text)")
        }
        .background(Capsule().fill(fill.color))
        .contentShape(Capsule())
        .onHover { hovered = $0 }
    }

    private var height: CGFloat { Theme.chipHeight * Platform.scale }

    private var fill: ThemeColor {
        if empty { return Tokens.warning.opacity(Opacity.colorTint) }
        if open { return Tokens.backgroundSecondaryAccentStronger }
        return hovered ? Tokens.backgroundSecondaryAccent : Tokens.backgroundSecondary
    }

    /// The x in the chip's round end: it hovers in a circle inside it.
    private struct RemoveFace: View {
        let empty: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            let side = Theme.chipHeight * Platform.scale
            Image(.x, size: 11)
                .foregroundStyle((hovered ? Tokens.foreground : empty ? Tokens.warning : Tokens.mutedStrongerForeground).color)
                .frame(width: side, height: side)
                .background(Circle().fill(hovered ? Tokens.backgroundSecondaryAccentStronger.color : .clear).padding(Space.xxs))
                .contentShape(Rectangle())
        }
    }
}

/// The chip that adds a condition to a filter: a dashed outline, a plus and its words. `open`
/// while what it opened is open: then its outline is drawn whole.
struct AddChip: View {
    let title: String
    var open = false
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(title: title, open: open) }
            .buttonStyle(.press)
    }

    private struct Face: View {
        let title: String
        let open: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            HStack(spacing: Space.xxs) {
                Image(.plus, size: 12)
                Text(title).font(.ui(.caption)).lineLimit(1)
            }
            .foregroundStyle((open || hovered ? Tokens.foreground : Tokens.mutedForeground).color)
            .padding(.horizontal, Space.s + 2)
            .frame(height: Theme.chipHeight * Platform.scale)
            .background(Capsule().fill(hovered || open ? Tokens.backgroundSecondary.color : .clear))
            .overlay(
                Capsule().strokeBorder(
                    (open ? Tokens.mutedForeground : Tokens.mutedStrongerForeground).color,
                    style: StrokeStyle(lineWidth: Theme.hairline, dash: open ? [] : [3, 2])
                )
            )
            .contentShape(Capsule())
        }
    }
}
