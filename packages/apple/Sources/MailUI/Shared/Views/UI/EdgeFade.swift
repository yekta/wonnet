import SwiftUI

extension View {
    /// What scrolls fades out over `length` at the top, instead of being cut off under what is
    /// above it. Pad the content by as much, so at rest none of it is faded.
    func fadesTop(_ length: CGFloat = Space.xl) -> some View {
        mask {
            VStack(spacing: 0) {
                LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom).frame(height: length)
                Rectangle()
            }
        }
    }
}
