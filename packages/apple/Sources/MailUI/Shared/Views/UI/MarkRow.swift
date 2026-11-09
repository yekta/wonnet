import SwiftUI

/// One of a few choices on a card, picked with a click anywhere on it: a round mark when only one
/// of them can be (`.one`), a square one when any can (`.any`), filled in the primary colour with
/// a check once picked. Then a symbol or an account's dot, its name and a line about it. The row
/// hovers in the card's accent; rows go one under the other with a hairline between them. An
/// `indent` sets a row under the one it belongs to, its fill still across the card. A `locked` row
/// is always so: a lock stands in its mark's place, and it can't be clicked or hovered.
struct MarkRow: View {
    enum Kind {
        case one, any
    }

    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    var kind: Kind = .one
    var indent: CGFloat = 0
    var locked = false
    let checked: Bool
    let action: () -> Void

    var body: some View {
        if locked {
            Face(title: title, detail: detail, symbol: symbol, dot: dot, kind: kind, indent: indent, checked: checked, locked: true)
                .accessibilityAddTraits(.isSelected)
        } else {
            Button(action: action) {
                Face(title: title, detail: detail, symbol: symbol, dot: dot, kind: kind, indent: indent, checked: checked, locked: false)
            }
            .buttonStyle(.press)
            .accessibilityAddTraits(checked ? .isSelected : [])
        }
    }

    private struct Face: View {
        let title: String
        let detail: String?
        let symbol: Symbol?
        let dot: Color?
        let kind: Kind
        let indent: CGFloat
        let checked: Bool
        let locked: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            HStack(alignment: .top, spacing: Space.s + 2) {
                if locked {
                    Image(.lock, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color)
                        .frame(width: Theme.selectCircle * Platform.scale, height: line)
                } else {
                    Mark(kind: kind, checked: checked, hovered: hovered)
                }
                if let dot {
                    Circle().fill(dot).frame(width: 8, height: 8).frame(width: 16, height: line)
                } else if let symbol {
                    Image(symbol, size: 15).foregroundStyle(Tokens.mutedForeground.color).frame(width: 16, height: line)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).textStyle(.body).frame(minHeight: line)
                    if let detail {
                        Text(detail).textStyle(.caption).fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Space.l)
            .padding(.leading, indent)
            .padding(.vertical, Space.s + 2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered ? Tokens.cardAccent.color : .clear)
            .contentShape(Rectangle())
        }

        /// The height of the name's line, which the mark and the symbol stand in the middle of.
        private var line: CGFloat { 20 * Platform.scale }
    }

    private struct Mark: View {
        let kind: Kind
        let checked: Bool
        let hovered: Bool

        var body: some View {
            let side = Theme.selectCircle * Platform.scale
            let shape = RoundedRectangle(cornerRadius: kind == .one ? side / 2 : Radius.xs)
            ZStack {
                if checked {
                    shape.fill(Tokens.primary.color)
                    Image(.check, size: side * 0.67).foregroundStyle(Tokens.primaryForeground.color)
                } else {
                    shape.strokeBorder((hovered ? Tokens.mutedForeground : Tokens.mutedStrongerForeground).color, lineWidth: Theme.selectCircleLine)
                }
            }
            .frame(width: side, height: side)
            .frame(height: 20 * Platform.scale)
        }
    }
}
