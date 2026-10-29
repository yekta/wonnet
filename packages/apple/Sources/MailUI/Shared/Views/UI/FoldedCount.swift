import SwiftUI

/// The messages of a long thread tucked away between its first and its last few, as one line
/// in place of theirs: their count in a circle where their avatars would be. It shows them.
struct FoldedCount: View {
    let count: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(count: count) }
            .buttonStyle(.press)
            .help("Show \(count) earlier messages")
    }

    private struct Face: View {
        let count: Int
        @Environment(\.hovered) private var hovered
        @Environment(\.onCard) private var onCard

        var body: some View {
            HStack(spacing: Space.m) {
                Text("\(count)")
                    .textStyle(.labelStrong, color: Tokens.mutedForeground.color)
                    .frame(width: side, height: side)
                    .background(Circle().fill(hovered ? accent.color : .clear))
                    .overlay(Circle().strokeBorder((onCard ? Theme.cardRule : Tokens.border).color, lineWidth: Theme.hairline))
                Text("\(count) earlier messages")
                    .textStyle(.body, color: hovered ? Tokens.foreground.color : Tokens.mutedStrongerForeground.color)
                Spacer(minLength: 0)
            }
            .padding(.vertical, Space.m)
            .contentShape(Rectangle())
        }

        private var side: CGFloat { Theme.messageAvatar * Platform.scale }
        private var accent: ThemeColor { onCard ? Theme.cardAccent : Tokens.backgroundAccent }
    }
}
