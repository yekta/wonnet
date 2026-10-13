import SwiftUI

/// A line of a message's head: its label at the start, what it holds, and a hairline under it,
/// inset as the card's content is. `wraps` is a line that grows with its chips, with the label
/// kept at the top.
struct FieldRow<Content: View>: View {
    /// From the card's edge to where the content starts, past the label.
    static var contentInset: CGFloat { Theme.cardInset + labelWidth + Space.s + 2 }
    static var labelWidth: CGFloat { 56 }

    let label: String
    var wraps = false
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: wraps ? .top : .center, spacing: Space.s + 2) {
                Text(label).textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                    .frame(width: Self.labelWidth, height: wraps ? Theme.chipHeight * Platform.scale : nil, alignment: .leading)
                content
            }
            .font(.ui(.body))
            .foregroundStyle(Tokens.foreground.color)
            .padding(.horizontal, Theme.cardInset)
            .padding(.vertical, wraps ? Space.s : 0)
            .frame(minHeight: Theme.searchHeight * Platform.scale)
            Rule(color: Tokens.borderCard).padding(.horizontal, Theme.cardInset)
        }
    }
}
