import SwiftUI

/// A field that narrows a list: the search icon, the words, and a way to clear them. `.sheet`
/// sits on a hairline at the edge of a sheet, given by `SearchedList`, and takes the keys that
/// move through the list (↑, ↓, Enter to pick, Esc to leave); `.bar` is the capsule of the top bar.
struct SearchField: View {
    enum Style {
        case sheet, bar
    }

    /// Where a `.sheet` field sits, over its list or under it.
    #if os(macOS)
    static let sheetEdge = VerticalEdge.top
    #else
    static let sheetEdge = VerticalEdge.bottom
    #endif

    let placeholder: String
    @Binding var text: String
    var style: Style = .sheet
    var symbol: Symbol = .search
    var autofocus = true
    /// Changed by the owner to put the cursor in the field again.
    var focusTrigger = 0
    var submit: () -> Void = {}
    var move: (Int) -> Void = { _ in }
    var escape: () -> Void = {}
    var clear: () -> Void = {}
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: Space.s) {
            Image(symbol, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color)
            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(.ui(style == .sheet ? 15 : 13))
                .foregroundStyle(Tokens.foreground.color)
                .focused($focused)
                .onSubmit(submit)
                .onListKeys(move: move, escape: leave)
                #if os(iOS)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                #endif
            if !text.isEmpty {
                PlainButton(help: "Clear", action: clearText) {
                    Image(.x, size: 12).foregroundStyle(Tokens.mutedStrongerForeground.color)
                }
            }
        }
        .padding(.horizontal, Space.m)
        .frame(height: (style == .sheet ? Theme.searchHeight : ControlSize.regular.height) * Platform.scale)
        .background { if style == .bar { Capsule().strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline) } }
        .overlay(alignment: Self.sheetEdge == .top ? .bottom : .top) {
            if style == .sheet { Rule(color: Tokens.borderCard) }
        }
        .onAppear { if autofocus { focused = true } }
        .onChange(of: focusTrigger) { focused = true }
    }

    private func clearText() {
        text = ""
        clear()
    }

    private func leave() {
        escape()
        guard style == .bar else { return }
        clearText()
        focused = false
    }
}

/// A sheet's list and the field that narrows it: over it on the Mac, under it on iOS, where the
/// thumb is and the keyboard comes up.
struct SearchedList<List: View>: View {
    let field: SearchField
    @ViewBuilder let list: List

    var body: some View {
        VStack(spacing: 0) {
            if SearchField.sheetEdge == .top { field }
            list
            if SearchField.sheetEdge == .bottom { field }
        }
    }
}
