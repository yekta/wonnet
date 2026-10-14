import SwiftUI

/// The circle that selects threads: empty, or filled in the primary colour with a check. The
/// Mac's rows draw the same circle themselves; this is the button, over the selection.
struct SelectCircle: View {
    let checked: Bool
    let help: String
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(checked: checked) }
            .buttonStyle(.press)
            .controlFace(ControlSize.regular.height)
            .help(help)
            .accessibilityLabel(help)
    }

    private struct Face: View {
        let checked: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            let side = Theme.selectCircle * Platform.scale
            let height = ControlSize.regular.height * Platform.scale
            ZStack {
                if checked {
                    Circle().fill(Tokens.primary.color)
                    Image(.check, size: side * 0.67).foregroundStyle(Tokens.primaryForeground.color)
                } else {
                    Circle().strokeBorder((hovered ? Tokens.foreground : Tokens.border).color, lineWidth: Theme.selectCircleLine)
                }
            }
            .frame(width: side, height: side)
            .frame(width: height, height: height)
            .background(Circle().fill(hovered ? Tokens.backgroundAccent.color : .clear))
            .padding(.horizontal, Theme.buttonGap / 2)
            .contentShape(Rectangle())
        }
    }
}
