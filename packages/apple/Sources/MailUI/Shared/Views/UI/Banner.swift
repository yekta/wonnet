import SwiftUI

/// A card floating at the bottom of the window that stays until what it says is done: its icon
/// in a wash of `tint`, a title over one line, the one thing to do about it, and `close` to put
/// it away. With `progress`, the line is a bar in `tint` beside `detail`. Every banner is the
/// same height, so one following another doesn't jump.
struct Banner: View {
    let symbol: Symbol
    var tint: ThemeColor = Tokens.mutedForeground
    let title: String
    let detail: String
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
                line
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
        .padding(.trailing, close == nil ? Space.m - Theme.buttonGap / 2 : Space.xxs)
        .padding(.vertical, Space.m)
        .frame(width: Banner.width * Platform.scale)
        .background(RoundedRectangle(cornerRadius: Radius.lg).fill(Tokens.popover.color).shadow(.lg))
        .overlay(RoundedRectangle(cornerRadius: Radius.lg).strokeBorder(Tokens.borderPopover.color, lineWidth: Theme.hairline))
        .environment(\.onPopover, true)
    }

    @ViewBuilder private var line: some View {
        let text = Text(detail).textStyle(.caption).lineLimit(1)
        if let progress {
            HStack(spacing: Space.s) {
                bar(progress)
                // As wide as the widest it gets, so the bar doesn't move as it fills.
                ZStack(alignment: .trailing) {
                    Text("100%").textStyle(.caption).hidden()
                    text
                }
                .monospacedDigit()
            }
        } else {
            text.help(detail)
        }
    }

    private var badge: some View {
        ZStack {
            Circle().fill(tint.opacity(Opacity.colorTint).color)
            if pending {
                Spinner(size: 16, tint: tint.color)
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
