import SwiftUI

/// The foot of the thread list while its next page is being read.
struct LoadingMore: View {
    static let height = Theme.macRowHeight * Platform.scale

    var body: some View {
        HStack(spacing: Space.s) {
            ProgressView().controlSize(.mini)
            Text("Loading more").textStyle(.caption)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
