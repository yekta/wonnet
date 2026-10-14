import SwiftUI

/// The spinner that shows something is still coming: Lucide's loader turning once a second, in
/// the muted text colour unless given a `tint`. A mailbox syncing, a page loading, a button
/// pending. `size` is the icon's, as `Image`'s.
struct Spinner: View {
    var size: CGFloat = 13
    var tint = Tokens.mutedForeground.color

    var body: some View {
        TimelineView(.animation) { context in
            Image(.loader, size: size)
                .foregroundStyle(tint)
                .rotationEffect(.degrees(context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1) * 360))
        }
    }
}
