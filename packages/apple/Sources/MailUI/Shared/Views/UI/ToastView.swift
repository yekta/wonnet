import SwiftUI

/// The note at the bottom: a line of text and maybe an action, gone after a while. A toast that
/// counts down (a message about to be sent) starts with a ring that fills until it is done.
/// The ring and the action are capsules inset from the toast's edge by as much on every side,
/// so their ends are concentric with the toast's.
struct ToastView: View {
    @Environment(MailStore.self) private var store

    private static let height = Theme.searchHeight * Platform.scale
    private static let inset = Space.xs * Platform.scale
    private static let inner = height - inset * 2

    var body: some View {
        if let toast = store.toast {
            HStack(spacing: 0) {
                if toast.countdown {
                    Countdown(start: toast.shown, duration: toast.duration).frame(width: Self.inner, height: Self.inner)
                }
                Text(toast.message)
                    .textStyle(.label, color: Tokens.foreground.color)
                    .padding(.leading, toast.countdown ? Space.s + 2 : Space.m)
                    .padding(.trailing, toast.action == nil ? Space.m : Space.xl)
                if let action = toast.action {
                    Button(action: store.toastTapped) { ActionFace(title: action, height: Self.inner) }.buttonStyle(.press)
                }
            }
            .padding(Self.inset)
            .frame(height: Self.height)
            .background(Capsule().fill(Tokens.popover.color).shadow(.md))
            .overlay(Capsule().strokeBorder(Tokens.borderPopover.color, lineWidth: Theme.hairline))
            .environment(\.onPopover, true)
            .padding(.bottom, Space.xl)
            .task(id: toast.id) {
                try? await Task.sleep(for: .seconds(toast.duration))
                store.dismissToast(toast)
            }
        }
    }

    /// The toast's action: an outlined capsule, as `ActionButton`'s `outline`, that fills the
    /// toast's height but for the inset.
    private struct ActionFace: View {
        let title: String
        let height: CGFloat
        @Environment(\.hovered) private var hovered

        var body: some View {
            Text(title)
                .textStyle(.labelStrong, color: Tokens.primary.color)
                .padding(.horizontal, Space.l)
                .frame(minWidth: height * 1.6, minHeight: height)
                .background(Capsule().fill(hovered ? Tokens.primary.opacity(Opacity.colorTint).color : .clear))
                .overlay(Capsule().strokeBorder(Tokens.primary.opacity(Opacity.colorBorder).color, lineWidth: Theme.hairline))
                .contentShape(Capsule())
        }
    }

    /// The seconds left, in a ring that fills as they run out.
    private struct Countdown: View {
        let start: Date
        let duration: TimeInterval

        var body: some View {
            TimelineView(.animation) { context in
                let left = max(0, duration - context.date.timeIntervalSince(start))
                ZStack {
                    Circle().stroke(Tokens.popoverAccentStronger.color, lineWidth: Self.line)
                    Circle()
                        .trim(from: 0, to: duration > 0 ? 1 - left / duration : 1)
                        .stroke(Tokens.foreground.color, style: StrokeStyle(lineWidth: Self.line, lineCap: .round))
                        .rotationEffect(.degrees(-90))
                    Text("\(Int(left.rounded(.up)))").monospacedDigit().textStyle(.labelStrong, color: Tokens.foreground.color)
                }
                .padding(Self.line / 2)
            }
            .accessibilityElement(children: .combine)
        }

        private static let line: CGFloat = 2.5
    }
}
