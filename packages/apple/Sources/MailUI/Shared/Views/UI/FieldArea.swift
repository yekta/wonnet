import SwiftUI

/// Laid behind a field's box, it makes the whole box the field, as on the web: a click anywhere on
/// it, its padding included, puts the caret in the field, and the pointer is the I-beam over all
/// of it. The system's text field is only as big as its line of text.
struct FieldArea: View {
    let focus: () -> Void

    var body: some View {
        Color.clear.contentShape(Rectangle()).onTapGesture(perform: focus).textPointer()
    }
}

extension View {
    /// The I-beam over a view, where the system can show it.
    @ViewBuilder func textPointer() -> some View {
        #if os(macOS)
        if #available(macOS 15, *) {
            pointerStyle(.horizontalText)
        } else {
            self
        }
        #else
        self
        #endif
    }
}
