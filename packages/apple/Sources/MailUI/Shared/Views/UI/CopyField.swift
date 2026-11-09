import SwiftUI

/// Text to take elsewhere, in a field's box that isn't typed in, with the button beside it that
/// copies it: the MCP server's address. It stands on the page, not on a card. The button shows a
/// check for a moment once it has.
struct CopyField: View {
    let text: String
    @State private var copied = false

    var body: some View {
        HStack(spacing: Space.s - Theme.buttonGap / 2) {
            Text(text)
                .textStyle(.body)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .padding(.horizontal, Space.s + 2)
                .frame(maxWidth: .infinity, minHeight: Theme.fieldHeight * Platform.scale, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: Radius.sm).fill(Tokens.input.color))
                .overlay(RoundedRectangle(cornerRadius: Radius.sm).strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline))
            ActionButton(title: copied ? "Copied" : "Copy", symbol: copied ? .check : .copy, variant: .outline, iconOnly: true) { copy() }
                .padding(.trailing, -Theme.buttonGap / 2)
        }
    }

    private func copy() {
        Platform.copy(text)
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            copied = false
        }
    }
}
