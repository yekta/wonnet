import SwiftUI

/// The small spinner that shows something is still coming, in the muted text colour: a mailbox
/// syncing, a page loading, an update being fetched. `size` fixes the box it sits in.
struct Spinner: View {
    var size: CGFloat?

    var body: some View {
        ProgressView().controlSize(.mini).tint(Tokens.mutedForeground.color).frame(width: size, height: size)
    }
}
