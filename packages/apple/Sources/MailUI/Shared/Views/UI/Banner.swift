import SwiftUI

/// A card floating at the bottom of the window that stays until what it says is done: its icon
/// in a wash of `tint`, a title over a line, the bar of what is under way, the one thing to do
/// about it, and `close` to put it away.
struct Banner: View {
    let symbol: Symbol
    var tint: ThemeColor = Tokens.mutedForeground
    let title: String
    var detail: String?
    var progress: Double?
    var pending = false
    var action: ActionButton?
    var close: (() -> Void)?

    static let width: CGFloat = 400

    var body: some View {
        HStack(spacing: Space.m) {
            badge
            VStack(alignment: .leading, spacing: 2) {
                Text(title).textStyle(.labelStrong, color: Tokens.foreground.color).lineLimit(1)
                if let detail {
                    Text(detail).textStyle(.caption).fixedSize(horizontal: false, vertical: true)
                }
                if let progress {
                    bar(progress).padding(.top, Space.xs + 2)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 0) {
                action
                if let close {
                    IconButton(symbol: .x, help: "Close", size: .small, circled: false, quiet: true, action: close)
                }
            }
        }
        .padding(.leading, Space.m)
        .padding(.trailing, close == nil ? Space.m - Theme.buttonGap / 2 : Space.xs)
        .padding(.vertical, Space.m)
        .frame(width: Banner.width * Platform.scale)
        .background(RoundedRectangle(cornerRadius: Radius.lg).fill(Tokens.popover.color).shadow(.lg))
        .overlay(RoundedRectangle(cornerRadius: Radius.lg).strokeBorder(Tokens.borderPopover.color, lineWidth: Theme.hairline))
    }

    private var badge: some View {
        ZStack {
            Circle().fill(tint.opacity(Opacity.colorTint).color)
            if pending {
                Spinner()
            } else {
                Image(symbol, size: 16).foregroundStyle(tint.color)
            }
        }
        .frame(width: 34 * Platform.scale, height: 34 * Platform.scale)
    }

    private func bar(_ fraction: Double) -> some View {
        Capsule()
            .fill(Tokens.popoverAccent.color)
            .overlay(alignment: .leading) {
                GeometryReader { space in
                    Capsule().fill(tint.color).frame(width: space.size.width * min(max(fraction, 0), 1))
                }
            }
            .frame(height: 4)
            .animation(.linear(duration: 0.2), value: fraction)
    }
}
