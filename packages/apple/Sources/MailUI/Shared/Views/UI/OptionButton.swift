import SwiftUI

/// A wide button on a card, for one of a few ways to do something: a mark (a logo, an icon),
/// what it is, a line about it, and a chevron: the providers an account comes from.
struct OptionButton<Mark: View>: View {
    let title: String
    var detail: String?
    let action: () -> Void
    @ViewBuilder let mark: Mark

    var body: some View {
        Button(action: action) {
            Face(title: title, detail: detail, mark: mark)
        }
        .buttonStyle(.press)
    }

    private struct Face: View {
        let markSide: CGFloat = 24
        let title: String
        let detail: String?
        let mark: Mark
        @Environment(\.hovered) private var hovered

        var body: some View {
            HStack(spacing: Space.m) {
                mark.frame(width: markSide * Platform.scale, height: markSide * Platform.scale)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).textStyle(.bodyStrong).lineLimit(1)
                    if let detail {
                        Text(detail).textStyle(.caption).lineLimit(1)
                    }
                }
                Spacer(minLength: Space.s)
                Image(.chevronRight, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color)
            }
            .padding(.horizontal, Space.l)
            .frame(maxWidth: .infinity, minHeight: Theme.optionHeight * Platform.scale)
            .background(RoundedRectangle(cornerRadius: Radius.md).fill((hovered ? Tokens.cardAccent : Tokens.card).color))
            .overlay(RoundedRectangle(cornerRadius: Radius.md).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
            .contentShape(Rectangle())
        }
    }
}
