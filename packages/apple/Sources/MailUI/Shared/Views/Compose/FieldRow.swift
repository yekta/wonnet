import SwiftUI

/// A line of a message's head: its label at the start, what it holds, and a hairline under it,
/// inset as the card's content is. `wraps` is a line that grows with its chips, with the label
/// kept at the top. With `focus`, the whole line is the field: a click anywhere on it, the label
/// included, puts the caret in the field, and the pointer is the I-beam over all of it. The
/// system's text field is only as tall as its one line of text, so the line does this itself.
struct FieldRow<Content: View>: View {
    /// From the card's edge to where the content starts, past the label.
    static var contentInset: CGFloat { Theme.cardInset + labelWidth + Space.s + 2 }
    static var labelWidth: CGFloat { 56 }

    let label: String
    var wraps = false
    var focus: (() -> Void)?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: wraps ? .top : .center, spacing: Space.s + 2) {
                Text(label).textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                    .frame(width: Self.labelWidth, height: wraps ? Theme.chipHeight * Platform.scale : nil, alignment: .leading)
                    .allowsHitTesting(false)
                content
            }
            .font(.ui(.body))
            .foregroundStyle(Tokens.foreground.color)
            .padding(.horizontal, Theme.cardInset)
            .padding(.vertical, wraps ? Space.s : 0)
            .frame(minHeight: Theme.searchHeight * Platform.scale)
            .background {
                if let focus {
                    Color.clear.contentShape(Rectangle()).onTapGesture(perform: focus).textPointer()
                }
            }
            Rule(color: Theme.cardRule).padding(.horizontal, Theme.cardInset)
        }
    }
}

extension View {
    /// The I-beam over a view, where the system can show it.
    @ViewBuilder func textPointer() -> some View {
        #if os(macOS)
        if #available(macOS 15, *) {
            pointerStyle(.horizontalText)
        } else {
            self
        }
        #else
        self
        #endif
    }
}
