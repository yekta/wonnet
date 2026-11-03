import SwiftUI

/// A setting that is on or off, with a line saying what it does; the switch is the system's,
/// at the end of the row. A click anywhere on the row flips it; the row hovers as a large thing,
/// its fill reaching `Space.s` past its words and its switch.
struct ToggleRow: View {
    let title: String
    var detail: String?
    @Binding var isOn: Bool

    var body: some View {
        Button { isOn.toggle() } label: {
            Face(title: title, detail: detail, isOn: $isOn)
        }
        .buttonStyle(.press)
    }

    private struct Face: View {
        let title: String
        let detail: String?
        @Binding var isOn: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            HStack(alignment: .center, spacing: Space.l) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).textStyle(.body)
                    if let detail {
                        Text(detail).textStyle(.caption).fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: Space.s)
                Switch(title: title, isOn: $isOn)
            }
            .frame(minHeight: ControlSize.small.height * Platform.scale)
            .padding(.horizontal, Space.s)
            .background(RoundedRectangle(cornerRadius: Radius.md).fill(hovered ? Tokens.backgroundAccentLarger.color : .clear))
            .padding(.horizontal, -Space.s)
            .contentShape(Rectangle())
        }
    }
}

/// The system's switch alone, in the primary colour: for a row that says what it does itself.
struct Switch: View {
    /// What it turns on, for VoiceOver.
    let title: String
    @Binding var isOn: Bool

    var body: some View {
        Toggle(isOn: $isOn) { Text(title) }
            .labelsHidden()
            .toggleStyle(.switch)
            .tint(Tokens.primary.color)
            #if os(macOS)
            .controlSize(.small)
            #endif
    }
}
