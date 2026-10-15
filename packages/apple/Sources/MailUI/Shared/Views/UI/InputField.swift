import SwiftUI

/// A text field on a hairline, with its label above if it has one. With `lines` above one it is a text box
/// that many lines tall, where Return starts a new line. `commit` runs on Return and when it loses focus.
struct InputField: View {
    var label: String?
    @Binding var text: String
    var placeholder = ""
    var secure = false
    var lines = 1
    var autofocus = false
    var commit: (() -> Void)?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            if let label {
                Text(label).textStyle(.footnote).fontWeight(.medium)
            }
            Group {
                if secure {
                    SecureField(placeholder, text: $text)
                } else if lines > 1 {
                    TextEditor(text: $text)
                        .scrollContentBackground(.hidden)
                        .overlay(alignment: .topLeading) {
                            if text.isEmpty {
                                Text(placeholder).foregroundStyle(Tokens.mutedStrongerForeground.color).padding(.leading, 5).allowsHitTesting(false)
                            }
                        }
                        .padding(.vertical, Space.s)
                } else {
                    TextField(placeholder, text: $text)
                        #if os(iOS)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        #endif
                }
            }
            .textFieldStyle(.plain)
            .focused($focused)
            .font(.ui(.body))
            .foregroundStyle(Tokens.foreground.color)
            .padding(.horizontal, Space.s + 2)
            .frame(height: boxHeight * Platform.scale)
            .background { FieldArea { focused = true } }
            .background(RoundedRectangle(cornerRadius: Radius.sm).fill(Tokens.input.color))
            .overlay(RoundedRectangle(cornerRadius: Radius.sm).strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline))
            .controlFace(boxHeight, inset: 0)
        }
        .onSubmit { commit?() }
        .onChange(of: focused) { _, now in
            guard !now else { return }
            commit?()
        }
        .onAppear {
            guard autofocus else { return }
            DispatchQueue.main.async { focused = true }
        }
    }

    private var boxHeight: CGFloat {
        lines > 1 ? CGFloat(lines) * 18 + 16 : Theme.fieldHeight
    }
}
