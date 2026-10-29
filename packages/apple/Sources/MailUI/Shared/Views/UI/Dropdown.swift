import SwiftUI

/// A choice from a short list, under a label: which label, which sender. `none` is the
/// choice of nothing, shown when `selection` is nil.
struct Dropdown<Option: Hashable>: View {
    var label: String?
    let options: [Option]
    @Binding var selection: Option?
    var none: String?
    let title: (Option) -> String

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            if let label {
                Text(label).textStyle(.footnote, color: Tokens.mutedStrongerForeground.color).fontWeight(.medium)
            }
            FaceMenu {
                if let none {
                    Button(none) { selection = nil }
                }
                ForEach(options, id: \.self) { option in
                    Button {
                        selection = option
                    } label: {
                        if option == selection {
                            Label(title(option), systemImage: "checkmark")
                        } else {
                            Text(title(option))
                        }
                    }
                }
            } face: {
                DropdownFace(text: selection.map(title) ?? none ?? "")
            }
        }
    }
}

private struct DropdownFace: View {
    let text: String
    @Environment(\.hovered) private var hovered

    var body: some View {
        HStack(spacing: Space.s) {
            Text(text).textStyle(.body).lineLimit(1)
            Spacer(minLength: Space.s)
            Image(.chevronDown, size: 12).foregroundStyle(Tokens.mutedStrongerForeground.color)
        }
        .padding(.horizontal, Space.s + 2)
        .frame(height: Theme.fieldHeight * Platform.scale)
        .background(RoundedRectangle(cornerRadius: Radius.sm).fill(hovered ? Tokens.backgroundAccent.color : Tokens.input.color))
        .overlay(RoundedRectangle(cornerRadius: Radius.sm).strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline))
        .contentShape(RoundedRectangle(cornerRadius: Radius.sm))
    }
}

/// More under a line that opens it: Advanced.
struct Disclosure<Content: View>: View {
    let title: String
    @State private var open = false
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: Space.m) {
            ActionButton(title: title, symbol: open ? .chevronDown : .chevronRight, variant: .ghost, size: .small) {
                withAnimation(.easeOut(duration: 0.15)) { open.toggle() }
            }
            if open {
                content
            }
        }
    }
}
